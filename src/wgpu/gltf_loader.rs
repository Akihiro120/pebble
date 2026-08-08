//! One-shot glTF 2.0 loading — [`load_gltf`] parses geometry, a skeleton,
//! and animation clips out of a `.gltf`/`.glb` file. Deliberately **not**
//! part of the [`Asset`](crate::assets::upload::Asset)/[`LazyResource`](crate::assets::singleton_asset::LazyResource)
//! pipeline: those retry forever on `None`, which is right for "the backend
//! isn't ready yet" but wrong for "this file doesn't exist" or "this glTF
//! feature isn't supported" — conditions that can never resolve on their
//! own. Call `load_gltf` directly (in `main()`, a `.once()` system, wherever
//! you need it) and handle the `Result` like any other fallible I/O.
//!
//! Scoped to geometry + skeleton + animation only — materials and textures
//! are never read, even though the file may reference them; load your own
//! textures via [`TextureBuilder`](super::textures::TextureBuilder) and
//! write your own [`Material`](super::material::Material)/shader separately.
//! See [`ModelLoadError::UnsupportedFeature`] for the full list of glTF
//! features this doesn't handle (multiple skins, `CUBICSPLINE`
//! interpolation, sparse accessors, morph targets, non-indexed primitives).

use std::collections::{HashMap, HashSet};

use super::{
    animation::{AnimationClip, Interpolation, JointTrack, Keyframe},
    mesh::{Mesh, MeshBuilder, Vertex},
    skeleton::{Joint, Skeleton, Transform},
    skinned_mesh::{SkinnedMesh, SkinnedMeshBuilder, SkinnedVertex},
};

#[derive(Debug)]
pub enum ModelLoadError {
    Io(std::io::Error),
    /// Wraps `gltf::Error`'s `Display` output — the `gltf` crate stays an
    /// implementation detail, not exposed in this crate's own error type.
    Parse(String),
    /// A glTF feature this loader doesn't support, named specifically:
    /// `"more than one skin"`, `"CUBICSPLINE interpolation"`,
    /// `"sparse accessors"`, `"non-indexed primitives"`, `"morph targets"`.
    UnsupportedFeature(&'static str),
    /// A primitive or accessor is missing data this loader requires, e.g. a
    /// skinned primitive with no `JOINTS_0`/`WEIGHTS_0`.
    MissingData(String),
}

impl std::fmt::Display for ModelLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read model file: {e}"),
            Self::Parse(msg) => write!(f, "failed to parse glTF: {msg}"),
            Self::UnsupportedFeature(feature) => write!(f, "unsupported glTF feature: {feature}"),
            Self::MissingData(msg) => write!(f, "missing glTF data: {msg}"),
        }
    }
}

impl std::error::Error for ModelLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ModelLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Everything [`load_gltf`] extracted from one file. `skinned_meshes` are
/// primitives bound to the file's one skin; `static_meshes` are everything
/// else (rigid props, environment pieces in the same file) as the ordinary
/// [`Mesh`] — not padded with identity joint weights just to force them
/// through the skinned path.
pub struct LoadedModel {
    pub skinned_meshes: Vec<(String, SkinnedMesh)>,
    pub static_meshes: Vec<(String, Mesh)>,
    pub skeleton: Option<Skeleton>,
    pub animations: Vec<AnimationClip>,
}

/// Placeholder tangent handedness for primitives with no `TANGENT`
/// attribute — same convention already used by hand-written vertex data
/// elsewhere in this repo (see `examples/*/src/main.rs`).
const DEFAULT_TANGENT: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

/// Loads geometry, a skeleton, and animation clips from a glTF 2.0 file
/// (`.gltf` or `.glb` — both handled transparently). One-shot and
/// synchronous: call it directly, not through the asset pipeline (see the
/// module docs above for why).
///
/// Supports exactly one skin per file; a file with more than one returns
/// [`ModelLoadError::UnsupportedFeature`]. A joint whose real parent (in the
/// glTF scene graph) isn't itself one of the skin's joints is treated as a
/// [`Skeleton`] root — a documented v1 limitation, not an error: this
/// discards that ancestor's transform, which is wrong for a rig where the
/// ancestor has a non-identity transform (most standard exports, with a
/// plain identity-transform armature root, aren't affected).
///
/// `LINEAR`/`STEP` animation interpolation only — `CUBICSPLINE` is a hard
/// error rather than being silently misread (its accessors pack an
/// in-tangent/value/out-tangent triple per keyframe, not a plain value).
/// Sparse accessors and non-indexed primitives are also hard errors.
///
/// Everything below is one function rather than several: the `gltf` crate's
/// `Reader` type is built from a `get_buffer_data` closure whose lifetime
/// has to match the primitive/skin/channel it reads from exactly, which
/// only infers cleanly when the reading code stays inline (a named,
/// separately-compiled helper function would have to spell out that
/// lifetime relationship explicitly, and the two ends of it — a loop-local
/// glTF handle and a closure borrowing the outer `buffers` — can't actually
/// be unified across a function boundary this way).
pub fn load_gltf(path: &str) -> Result<LoadedModel, ModelLoadError> {
    let (document, buffers, _images) = gltf::import(path).map_err(|e| match e {
        gltf::Error::Io(io_err) => ModelLoadError::Io(io_err),
        other => ModelLoadError::Parse(other.to_string()),
    })?;
    let get_buffer_data = |buffer: gltf::Buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice());

    let skin = match document.skins().len() {
        0 => None,
        1 => document.skins().next(),
        _ => return Err(ModelLoadError::UnsupportedFeature("more than one skin")),
    };

    let skeleton = match &skin {
        Some(skin) => {
            let joint_nodes: Vec<gltf::Node> = skin.joints().collect();
            let node_to_joint: HashMap<usize, usize> =
                joint_nodes.iter().enumerate().map(|(i, node)| (node.index(), i)).collect();

            let inverse_bind_matrices: Vec<glam::Mat4> =
                match skin.reader(get_buffer_data).read_inverse_bind_matrices() {
                    Some(matrices) => matrices.map(|m| glam::Mat4::from_cols_array_2d(&m)).collect(),
                    // glTF allows omitting inverse bind matrices entirely (implies identity for every joint).
                    None => vec![glam::Mat4::IDENTITY; joint_nodes.len()],
                };
            if inverse_bind_matrices.len() != joint_nodes.len() {
                return Err(ModelLoadError::MissingData(format!(
                    "skin has {} joints but {} inverse bind matrices",
                    joint_nodes.len(),
                    inverse_bind_matrices.len(),
                )));
            }

            // Maps a child glTF node index to its parent's *joint-list*
            // index — by scanning every joint's own `children()` (glTF
            // nodes carry no back-pointer to their own parent, only forward
            // `children()` links). A joint whose real glTF-graph parent
            // isn't itself one of this skin's joints simply never appears
            // as a key here, and so becomes a Skeleton root — a documented
            // v1 limitation, see this function's doc comment.
            let mut parent_of_node: HashMap<usize, usize> = HashMap::new();
            for (parent_joint_index, joint_node) in joint_nodes.iter().enumerate() {
                for child in joint_node.children() {
                    if node_to_joint.contains_key(&child.index()) {
                        parent_of_node.insert(child.index(), parent_joint_index);
                    }
                }
            }

            let joints = joint_nodes
                .iter()
                .zip(inverse_bind_matrices)
                .map(|(node, inverse_bind_matrix)| {
                    let (translation, rotation, scale) = node.transform().decomposed();
                    Joint {
                        name: node.name().unwrap_or("joint").to_string(),
                        parent: parent_of_node.get(&node.index()).copied(),
                        inverse_bind_matrix,
                        local_bind_transform: Transform {
                            translation: glam::Vec3::from(translation),
                            rotation: glam::Quat::from_array(rotation),
                            scale: glam::Vec3::from(scale),
                        },
                    }
                })
                .collect();

            Some(Skeleton::new(joints))
        }
        None => None,
    };

    let skinned_node_indices: HashSet<usize> = match &skin {
        Some(skin) => document
            .nodes()
            .filter(|n| matches!(n.skin(), Some(s) if s.index() == skin.index()))
            .map(|n| n.index())
            .collect(),
        None => HashSet::new(),
    };

    let mut skinned_meshes = Vec::new();
    let mut static_meshes = Vec::new();
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        let is_skinned = skinned_node_indices.contains(&node.index());

        for (i, primitive) in mesh.primitives().enumerate() {
            let name = format!("{}_{i}", mesh.name().unwrap_or("mesh"));
            check_not_sparse(&primitive, gltf::Semantic::Positions)?;
            let reader = primitive.reader(get_buffer_data);

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| ModelLoadError::MissingData(format!("primitive '{name}' has no POSITION attribute")))?
                .collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .ok_or_else(|| ModelLoadError::MissingData(format!("primitive '{name}' has no NORMAL attribute")))?
                .collect();
            let tex_coords: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .ok_or_else(|| ModelLoadError::MissingData(format!("primitive '{name}' has no TEXCOORD_0 attribute")))?;
            let tangents: Vec<[f32; 4]> = match reader.read_tangents() {
                Some(t) => t.collect(),
                None => vec![DEFAULT_TANGENT; positions.len()],
            };
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|idx| idx.into_u32().collect())
                .ok_or(ModelLoadError::UnsupportedFeature("non-indexed primitives"))?;

            if is_skinned {
                check_not_sparse(&primitive, gltf::Semantic::Joints(0))?;
                check_not_sparse(&primitive, gltf::Semantic::Weights(0))?;
                let joints: Vec<[u16; 4]> = reader
                    .read_joints(0)
                    .map(|j| j.into_u16().collect())
                    .ok_or_else(|| ModelLoadError::MissingData(format!("skinned primitive '{name}' has no JOINTS_0 attribute")))?;
                let weights: Vec<[f32; 4]> = reader
                    .read_weights(0)
                    .map(|w| w.into_f32().collect())
                    .ok_or_else(|| ModelLoadError::MissingData(format!("skinned primitive '{name}' has no WEIGHTS_0 attribute")))?;

                let vertices = positions
                    .into_iter()
                    .zip(normals)
                    .zip(tex_coords)
                    .zip(tangents)
                    .zip(joints)
                    .zip(weights)
                    .map(|(((((p, n), uv), t), j), w)| {
                        SkinnedVertex::new(
                            glam::Vec3::from(p),
                            glam::Vec2::from(uv),
                            glam::Vec3::from(n),
                            glam::Vec4::from(t),
                            j,
                            w,
                        )
                    })
                    .collect();
                skinned_meshes.push((name, SkinnedMeshBuilder::new(vertices, indices).build()));
            } else {
                let vertices = positions
                    .into_iter()
                    .zip(normals)
                    .zip(tex_coords)
                    .zip(tangents)
                    .map(|(((p, n), uv), t)| {
                        Vertex::new(glam::Vec3::from(p), glam::Vec2::from(uv), glam::Vec3::from(n), glam::Vec4::from(t))
                    })
                    .collect();
                static_meshes.push((name, MeshBuilder::new(vertices, indices).build()));
            }
        }
    }

    let animations = match &skeleton {
        Some(skeleton) => {
            let mut clips = Vec::new();
            for animation in document.animations() {
                let mut tracks: HashMap<usize, JointTrack> = HashMap::new();

                for channel in animation.channels() {
                    let node = channel.target().node();
                    let Some(joint_index) = node
                        .name()
                        .and_then(|name| (0..skeleton.joint_count()).find(|&i| skeleton.joint(i).name == name))
                    else {
                        // Channel targets a node that isn't one of this
                        // skeleton's joints (a camera, a non-joint prop) —
                        // not animatable via Skeleton, so it's skipped.
                        continue;
                    };

                    let interpolation = match channel.sampler().interpolation() {
                        gltf::animation::Interpolation::Linear => Interpolation::Linear,
                        gltf::animation::Interpolation::Step => Interpolation::Step,
                        gltf::animation::Interpolation::CubicSpline => {
                            return Err(ModelLoadError::UnsupportedFeature("CUBICSPLINE interpolation"));
                        }
                    };

                    let reader = channel.reader(get_buffer_data);
                    let times: Vec<f32> = reader
                        .read_inputs()
                        .ok_or_else(|| ModelLoadError::MissingData("animation channel has no keyframe times".to_string()))?
                        .collect();
                    let outputs = reader.read_outputs().ok_or_else(|| {
                        ModelLoadError::MissingData("animation channel has no keyframe values".to_string())
                    })?;

                    let track = tracks.entry(joint_index).or_insert_with(|| JointTrack {
                        joint_index,
                        translation: Vec::new(),
                        translation_interpolation: Interpolation::Linear,
                        rotation: Vec::new(),
                        rotation_interpolation: Interpolation::Linear,
                        scale: Vec::new(),
                        scale_interpolation: Interpolation::Linear,
                    });

                    match outputs {
                        gltf::animation::util::ReadOutputs::Translations(values) => {
                            track.translation = times
                                .into_iter()
                                .zip(values)
                                .map(|(time, v)| Keyframe { time, value: glam::Vec3::from(v) })
                                .collect();
                            track.translation_interpolation = interpolation;
                        }
                        gltf::animation::util::ReadOutputs::Rotations(values) => {
                            track.rotation = times
                                .into_iter()
                                .zip(values.into_f32())
                                .map(|(time, v)| Keyframe { time, value: glam::Quat::from_array(v) })
                                .collect();
                            track.rotation_interpolation = interpolation;
                        }
                        gltf::animation::util::ReadOutputs::Scales(values) => {
                            track.scale = times
                                .into_iter()
                                .zip(values)
                                .map(|(time, v)| Keyframe { time, value: glam::Vec3::from(v) })
                                .collect();
                            track.scale_interpolation = interpolation;
                        }
                        gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => {
                            return Err(ModelLoadError::UnsupportedFeature("morph targets"));
                        }
                    }
                }

                clips.push(AnimationClip::new(
                    animation.name().unwrap_or("animation").to_string(),
                    tracks.into_values().collect(),
                ));
            }
            clips
        }
        None => {
            let animation_count = document.animations().len();
            if animation_count > 0 {
                tracing::warn!(
                    "load_gltf: file has {animation_count} animation(s) but no skin — skipping, \
                     nothing to animate"
                );
            }
            Vec::new()
        }
    };

    Ok(LoadedModel { skinned_meshes, static_meshes, skeleton, animations })
}

fn check_not_sparse(primitive: &gltf::Primitive, semantic: gltf::Semantic) -> Result<(), ModelLoadError> {
    if let Some(accessor) = primitive.get(&semantic)
        && accessor.sparse().is_some()
    {
        return Err(ModelLoadError::UnsupportedFeature("sparse accessors"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/gltf/two_joint_skeleton.gltf").to_string()
    }

    #[test]
    fn loads_geometry_skeleton_and_animation_from_a_hand_authored_fixture() {
        let model = load_gltf(&fixture_path()).expect("fixture should load cleanly");

        assert_eq!(model.static_meshes.len(), 0);
        // One primitive on the one skinned mesh node — SkinnedMesh's fields
        // are private (see skinned_mesh.rs), so this just confirms load_gltf
        // produced exactly one, not its internal vertex/index counts.
        assert_eq!(model.skinned_meshes.len(), 1);

        let skeleton = model.skeleton.expect("fixture has one skin, expected a Skeleton");
        assert_eq!(skeleton.joint_count(), 3);
        // Scrambled input order (child=0, root=1, mid=2) must still resolve
        // correctly: "root" has no parent, "mid"'s parent is "root", "child"'s
        // parent is "mid".
        let root = skeleton.joint_index_by_name("root").unwrap();
        let mid = skeleton.joint_index_by_name("mid").unwrap();
        let child = skeleton.joint_index_by_name("child").unwrap();
        assert_eq!(skeleton.joint(root).parent, None);
        assert_eq!(skeleton.joint(mid).parent, Some(root));
        assert_eq!(skeleton.joint(child).parent, Some(mid));

        assert_eq!(model.animations.len(), 1);
        let clip = &model.animations[0];
        assert_eq!(clip.name, "wave");
        assert_eq!(clip.duration, 1.0);

        // Sampling at the midpoint should linearly interpolate the animated
        // root joint's translation from (0,0,0) to (0,0,5).
        let poses = clip.sample(0.5, &skeleton);
        assert!((poses[root].translation.z - 2.5).abs() < 1e-5);
    }
}
