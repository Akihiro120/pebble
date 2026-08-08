//! Loads a small hand-authored glTF file (a 4-joint chain with a quad-strip
//! "flag" mesh skinned to it, plus one sway animation) and renders it with
//! a hand-written WGSL skinning shader — the full loop this example exists
//! to prove: `load_gltf` hands back plain data (a `SkinnedMesh`, a
//! `Skeleton`, an `AnimationClip`), you build the GPU resources and write
//! the shader yourself. Nothing here is auto-rendered.

use std::sync::Arc;

use pebble::prelude::*;
use pebble::wgpu::prelude::*;
use pebble::wgpu::{
    animation::AnimationClip,
    backend::{WGPUBackend, WGPUPlugin},
    gltf_loader::{load_gltf, LoadedModel},
    instance::{GPUMaterialInstance, MaterialInstance, MaterialInstanceBuilder},
    material::{GPUMaterial, Material, MaterialBuilder},
    skeleton::Skeleton,
    skinned_mesh::{GPUSkinnedMesh, SkinnedMesh, SkinnedVertex},
};

const SHADER: &str = r#"
struct VOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(8) joint_indices: vec4<u32>,
    @location(9) joint_weights: vec4<f32>,
) -> VOut {
    // The actual skinning math: blend each joint's current matrix by this
    // vertex's weight for it. pebble hands back the matrix palette
    // (Skeleton::skinning_matrices) and this vertex buffer's joint
    // indices/weights (SkinnedVertex) — the blend itself is ordinary WGSL,
    // same as any other engine's skinning shader.
    let skin =
        joint_matrices[joint_indices.x] * joint_weights.x +
        joint_matrices[joint_indices.y] * joint_weights.y +
        joint_matrices[joint_indices.z] * joint_weights.z +
        joint_matrices[joint_indices.w] * joint_weights.w;
    let world_pos = skin * vec4<f32>(position, 1.0);

    // No camera in this example — just enough of an orthographic-style
    // scale/offset to fit the flag's bind-pose extent (X: 0..3, Y: -0.4..0.4)
    // into clip space.
    var out: VOut;
    out.clip_pos = vec4<f32>((world_pos.xy - vec2<f32>(1.5, 0.0)) * vec2<f32>(0.6, 1.5), 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.uv.x, 0.4, 1.0 - in.uv.x, 1.0);
}
"#;

/// Per-entity animation playback state — this is entirely your own code,
/// not part of `pebble::wgpu`; `Skeleton`/`AnimationClip` are just data, you
/// decide how to drive them frame to frame.
struct AnimationState {
    skeleton: Arc<Skeleton>,
    clip: Arc<AnimationClip>,
    time: f32,
}

fn main() {
    tracing_subscriber::fmt::init();

    // A one-shot, synchronous call — not part of the asset pipeline, so a
    // bad path or malformed file is a real Err you handle right here, not a
    // silent forever-retry.
    let loaded = match load_gltf("../assets/models/simple_character.gltf") {
        Ok(model) => model,
        Err(e) => {
            eprintln!("failed to load model: {e}");
            std::process::exit(1);
        }
    };

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Skeletal Animation".to_string(),
            width: 1280,
            height: 720,
        }))
        .add_resource(loaded)
        .add_system(SystemStage::PreUpdate, setup.once())
        .add_system(SystemStage::Update, advance_animation)
        .add_system(SystemStage::Update, update_joint_buffers)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut model: ResMut<LoadedModel>,
    mut skinned_meshes: ResMut<Assets<SkinnedMesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut instances: ResMut<Assets<MaterialInstance>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let skeleton = Arc::new(model.skeleton.take()?);
    let clip = Arc::new(model.animations.pop()?);
    let (name, mesh) = model.skinned_meshes.pop()?;
    let mesh_handle = skinned_meshes.insert(&name, mesh);

    let material = MaterialBuilder::new(SHADER)
        .label("skinned")
        .vertex_layouts(vec![SkinnedVertex::layout()])
        .entries(vec![GroupEntry::Own(vec![BindingEntry {
            name: "joint_matrices",
            binding: 0,
            kind: BindingKind::storage_buffer_read_only(ShaderStages::VERTEX),
        }])])
        .no_cull_mode()
        .targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: None,
            write_mask: ColorWrites::ALL,
        }])
        .build_asset("skinned", &mut materials);

    // Bind-pose matrices as the initial contents — update_joint_buffers
    // overwrites this with the real animated pose before the first draw.
    let initial_matrices = vec![glam::Mat4::IDENTITY; skeleton.joint_count()];
    let instance = MaterialInstanceBuilder::new(material)
        .storage("joint_matrices", bytemuck::cast_slice(&initial_matrices).to_vec())
        .build_asset("flag_instance", &mut instances);

    commands.spawn((mesh_handle, instance, AnimationState { skeleton, clip, time: 0.0 }));
    Some(())
}

/// Advances every animated entity's own clock. `AnimationClip::sample`
/// clamps rather than loops, so looping — `rem_euclid` here — is this
/// system's job, not the clip's.
fn advance_animation(time: Res<Time>, mut query: Query<&mut AnimationState>) {
    for state in query.iter() {
        state.time = (state.time + time.delta_seconds()).rem_euclid(state.clip.duration);
    }
}

/// Samples the clip, turns the resulting per-joint poses into a skinning
/// matrix palette, and rewrites the instance's storage buffer with it —
/// `GPUBindingInstance::update` is the same primitive any other
/// per-frame-rewritten instance buffer would use, nothing skinning-specific
/// about it.
fn update_joint_buffers(
    mut query: Query<(&AnimationState, &Handle<MaterialInstance>)>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
) {
    for (state, instance_handle) in query.iter() {
        let Some(instance) = instances.get(instance_handle.id) else { continue };
        let poses = state.clip.sample(state.time, &state.skeleton);
        let matrices = state.skeleton.skinning_matrices(&poses);
        instance.update("joint_matrices", bytemuck::cast_slice(&matrices));
    }
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUSkinnedMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<SkinnedMesh>, &Handle<MaterialInstance>)>,
) {
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for (mesh_handle, instance_handle) in query.iter() {
        let Some(mesh) = meshes.get(mesh_handle.id) else { continue };
        let Some(instance) = instances.get(instance_handle.id) else { continue };
        let Some(material) = materials.get(instance.target) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, &instance.bind_group, &[]);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
