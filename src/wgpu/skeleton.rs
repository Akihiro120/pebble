//! Plain CPU data for a joint hierarchy — no [`Asset`](crate::assets::upload::Asset)/
//! `Handle`/GPU upload involved. A skeleton is pure computation (walking a
//! joint hierarchy, multiplying matrices) right up until you write the
//! result into a buffer of your own — see [`Skeleton::skinning_matrices`].

/// A local (parent-relative) rigid pose — translation, rotation, scale.
///
/// Kept as separate T/R/S rather than a single `glam::Mat4`: interpolating
/// a matrix directly (e.g. lerping its columns) is mathematically wrong —
/// rotation has to slerp/nlerp, not lerp component-wise — so
/// [`AnimationClip::sample`](super::animation::AnimationClip::sample) needs
/// T/R/S kept apart to interpolate each correctly, and only composes them
/// into a matrix at the very end via [`to_matrix`](Self::to_matrix).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: glam::Vec3::ZERO,
        rotation: glam::Quat::IDENTITY,
        scale: glam::Vec3::ONE,
    };

    pub fn to_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Blends toward `other` by `t` (`0.0` = `self`, `1.0` = `other`) —
    /// translation/scale lerp, rotation slerp. The building block for
    /// crossfading between two animations: sample both clips into a
    /// `Vec<Transform>` each, then `poses_a.iter().zip(&poses_b).map(|(a,
    /// b)| a.lerp(b, t)).collect()`. Blending more than two poses, per-bone
    /// blend masks, and additive blending are all just repeated/weighted
    /// applications of this same building block — left to you, since the
    /// right blending strategy depends entirely on what you're building.
    pub fn lerp(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            translation: self.translation.lerp(other.translation, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One joint's static rig data — everything that never changes once the
/// skeleton is built, as opposed to [`Transform`], which is per-pose (a new
/// one every frame, from [`AnimationClip::sample`](super::animation::AnimationClip::sample)).
#[derive(Clone, Debug)]
pub struct Joint {
    /// For lookup via [`Skeleton::joint_index_by_name`] and diagnostics —
    /// has no effect on the math.
    pub name: String,
    /// Index into the same [`Skeleton`]'s joint list. `None` for a root
    /// joint (no parent within this skeleton).
    pub parent: Option<usize>,
    /// Transforms a vertex from mesh-bind space into this joint's local
    /// space — the fixed per-joint matrix `Skeleton::skinning_matrices`
    /// multiplies each joint's current world matrix by.
    pub inverse_bind_matrix: glam::Mat4,
    /// This joint's own local transform in the bind pose — the fallback
    /// [`AnimationClip::sample`](super::animation::AnimationClip::sample)
    /// uses for any joint (or T/R/S component) a clip doesn't animate.
    pub local_bind_transform: Transform,
}

/// A joint hierarchy — parent/child relationships plus each joint's fixed
/// bind-pose data. Immutable once built; combine it with a per-frame
/// `Vec<Transform>` (one local pose per joint, e.g. from
/// [`AnimationClip::sample`](super::animation::AnimationClip::sample)) via
/// [`world_matrices`](Self::world_matrices)/[`skinning_matrices`](Self::skinning_matrices)
/// to get the matrices your own shader/buffer actually needs.
pub struct Skeleton {
    joints: Vec<Joint>,
    /// Precomputed once in [`new`](Self::new): indices into `joints`, with
    /// every joint appearing after its parent. glTF's own node array isn't
    /// guaranteed to already be in this order, so `world_matrices` doesn't
    /// assume `joints` itself is — it walks `topo_order` instead, which is.
    topo_order: Vec<usize>,
}

impl Skeleton {
    /// Panics if any `parent` index is out of range, or the joint graph
    /// contains a cycle — both are a malformed skeleton (a genuine bug in
    /// whatever built `joints`), not a "not ready yet" condition worth
    /// tolerating. Does not require `joints` to already be in
    /// parent-before-child order.
    pub fn new(joints: Vec<Joint>) -> Self {
        let len = joints.len();
        for (i, joint) in joints.iter().enumerate() {
            if let Some(parent) = joint.parent {
                assert!(
                    parent < len,
                    "Skeleton::new: joint {i} ('{}') has parent index {parent}, out of range for {len} joints",
                    joint.name,
                );
            }
        }

        let mut children: Vec<Vec<usize>> = vec![Vec::new(); len];
        let mut roots: Vec<usize> = Vec::new();
        for (i, joint) in joints.iter().enumerate() {
            match joint.parent {
                Some(parent) => children[parent].push(i),
                None => roots.push(i),
            }
        }

        // BFS from every root — a node's parent is always visited (and thus
        // pushed) before its children are, so this order already satisfies
        // "parent before child" with no extra sorting step.
        let mut topo_order = Vec::with_capacity(len);
        let mut queue = roots;
        while let Some(i) = queue.pop() {
            topo_order.push(i);
            queue.extend(children[i].iter().copied());
        }

        assert!(
            topo_order.len() == len,
            "Skeleton::new: joint graph has a cycle — {} of {len} joints are unreachable from any \
             root (a joint with no parent within this skeleton)",
            len - topo_order.len(),
        );

        Self { joints, topo_order }
    }

    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    pub fn joint(&self, index: usize) -> &Joint {
        &self.joints[index]
    }

    pub fn joint_index_by_name(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    /// Each joint's local bind transform — the rest pose, one entry per joint.
    /// Use as the starting point for IK: sample this, modify specific joints,
    /// then feed into [`skinning_matrices`](Self::skinning_matrices).
    pub fn bind_pose(&self) -> Vec<Transform> {
        self.joints.iter().map(|j| j.local_bind_transform).collect()
    }

    /// Computes each joint's world-space matrix from a set of local
    /// (parent-relative) poses — one linear pass over the precomputed
    /// topological order, no recursion needed. Panics if
    /// `local_poses.len() != self.joint_count()`.
    pub fn world_matrices(&self, local_poses: &[Transform]) -> Vec<glam::Mat4> {
        assert!(
            local_poses.len() == self.joints.len(),
            "Skeleton::world_matrices: {} local poses given for {} joints",
            local_poses.len(),
            self.joints.len(),
        );

        let mut world = vec![glam::Mat4::IDENTITY; self.joints.len()];
        for &i in &self.topo_order {
            let local = local_poses[i].to_matrix();
            world[i] = match self.joints[i].parent {
                Some(parent) => world[parent] * local,
                None => local,
            };
        }
        world
    }

    /// The matrix palette your shader/buffer actually needs: each joint's
    /// world matrix (from [`world_matrices`](Self::world_matrices)) times
    /// its own [`inverse_bind_matrix`](Joint::inverse_bind_matrix), so the
    /// result transforms a vertex straight from mesh-bind space into the
    /// current pose.
    pub fn skinning_matrices(&self, local_poses: &[Transform]) -> Vec<glam::Mat4> {
        let world = self.world_matrices(local_poses);
        world
            .iter()
            .zip(&self.joints)
            .map(|(w, joint)| *w * joint.inverse_bind_matrix)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(name: &str, parent: Option<usize>) -> Joint {
        Joint {
            name: name.to_string(),
            parent,
            inverse_bind_matrix: glam::Mat4::IDENTITY,
            local_bind_transform: Transform::IDENTITY,
        }
    }

    #[test]
    fn transform_lerp_blends_translation_scale_and_rotation() {
        let a = Transform {
            translation: glam::Vec3::new(0.0, 0.0, 0.0),
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::new(1.0, 1.0, 1.0),
        };
        let b = Transform {
            translation: glam::Vec3::new(10.0, 0.0, 0.0),
            rotation: glam::Quat::from_rotation_y(std::f32::consts::PI),
            scale: glam::Vec3::new(3.0, 3.0, 3.0),
        };

        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.translation, glam::Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(mid.scale, glam::Vec3::new(2.0, 2.0, 2.0));
        // Halfway through a 180-degree turn is a 90-degree turn.
        let angle = mid.rotation.to_axis_angle().1;
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5, "expected a ~90 degree rotation, got {angle}");
    }

    #[test]
    fn transform_lerp_at_the_endpoints_returns_each_transform_unchanged() {
        let a = Transform { translation: glam::Vec3::new(1.0, 2.0, 3.0), ..Transform::IDENTITY };
        let b = Transform { translation: glam::Vec3::new(4.0, 5.0, 6.0), ..Transform::IDENTITY };
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
    }

    #[test]
    fn out_of_range_parent_panics() {
        let result = std::panic::catch_unwind(|| Skeleton::new(vec![joint("root", Some(1))]));
        assert!(result.is_err(), "expected a panic for an out-of-range parent index");
    }

    #[test]
    fn a_two_joint_cycle_panics() {
        let result = std::panic::catch_unwind(|| {
            Skeleton::new(vec![joint("a", Some(1)), joint("b", Some(0))])
        });
        assert!(result.is_err(), "expected a panic for a joint cycle");
    }

    #[test]
    fn world_matrices_is_correct_even_when_input_order_is_not_topological() {
        // Deliberately listing the child (index 0) before its parent (index
        // 1) before the grandparent (index 2) — exactly the "glTF's node
        // array isn't guaranteed sorted" scenario Skeleton::new must handle.
        let joints = vec![
            joint("child", Some(1)),
            joint("mid", Some(2)),
            joint("root", None),
        ];
        let skeleton = Skeleton::new(joints);

        let poses = vec![
            Transform { translation: glam::Vec3::new(1.0, 0.0, 0.0), ..Transform::IDENTITY },
            Transform { translation: glam::Vec3::new(0.0, 1.0, 0.0), ..Transform::IDENTITY },
            Transform { translation: glam::Vec3::new(0.0, 0.0, 1.0), ..Transform::IDENTITY },
        ];
        let world = skeleton.world_matrices(&poses);

        // root: (0,0,1). mid: root * (0,1,0) = (0,1,1). child: mid * (1,0,0) = (1,1,1).
        assert_eq!(world[2].transform_point3(glam::Vec3::ZERO), glam::Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(world[1].transform_point3(glam::Vec3::ZERO), glam::Vec3::new(0.0, 1.0, 1.0));
        assert_eq!(world[0].transform_point3(glam::Vec3::ZERO), glam::Vec3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn skinning_matrices_applies_inverse_bind_matrix() {
        let mut root = joint("root", None);
        root.inverse_bind_matrix = glam::Mat4::from_translation(glam::Vec3::new(-2.0, 0.0, 0.0));
        let skeleton = Skeleton::new(vec![root]);

        let poses = vec![Transform {
            translation: glam::Vec3::new(5.0, 0.0, 0.0),
            ..Transform::IDENTITY
        }];
        let skinning = skeleton.skinning_matrices(&poses);

        // world = translate(5,0,0); skinning = world * inverse_bind = translate(3,0,0).
        assert_eq!(skinning[0].transform_point3(glam::Vec3::ZERO), glam::Vec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn world_matrices_panics_on_mismatched_pose_count() {
        let skeleton = Skeleton::new(vec![joint("root", None)]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            skeleton.world_matrices(&[])
        }));
        assert!(result.is_err(), "expected a panic for a local_poses length mismatch");
    }

    #[test]
    fn joint_index_by_name_finds_and_misses_correctly() {
        let skeleton = Skeleton::new(vec![joint("root", None), joint("child", Some(0))]);
        assert_eq!(skeleton.joint_index_by_name("child"), Some(1));
        assert_eq!(skeleton.joint_index_by_name("missing"), None);
    }
}
