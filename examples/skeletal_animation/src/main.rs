//! Loads a glTF file with a skinned mesh and plays its animations using
//! the `SkinnedBatchingPlugin` — the recommended approach for skinned
//! rendering in pebble. The plugin batches joint matrices for all animated
//! entities and writes them into a GPU storage buffer each frame.
//!
//! The WGSL shader receives joint matrices via the `"pebble_skinning"` global
//! bind group (binding 0: storage buffer of all matrices, binding 1: uniform
//! with joint count). Each vertex uses `instance_index * skin_info.joint_count`
//! as the base offset into the flat matrix array so instanced draw calls work.

use pebble::prelude::*;
use pebble::wgpu::prelude::*;
use pebble::wgpu::{
    backend::{WGPUBackend, WGPUPlugin},
    material::{Material, MaterialBuilder},
    skinned_mesh::{SkinnedMesh, SkinnedMeshBuilder, SkinnedVertex},
};

const SHADER: &str = r#"
struct SkinningInfo { joint_count: u32 }

@group(0) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;
@group(0) @binding(1) var<uniform>        skin_info:     SkinningInfo;

struct VOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
    @location(0) position:      vec3<f32>,
    @location(1) uv:            vec2<f32>,
    @location(8) joint_indices: vec4<u32>,
    @location(9) joint_weights: vec4<f32>,
) -> VOut {
    let base = instance_index * skin_info.joint_count;
    let skin =
        joint_matrices[base + joint_indices.x] * joint_weights.x +
        joint_matrices[base + joint_indices.y] * joint_weights.y +
        joint_matrices[base + joint_indices.z] * joint_weights.z +
        joint_matrices[base + joint_indices.w] * joint_weights.w;
    let world_pos = skin * vec4<f32>(position, 1.0);

    // No camera in this example — a fixed orthographic-style scale/offset
    // to fit the flag's bind-pose extent (X: 0..3, Y: -0.4..0.4) into clip space.
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

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Skeletal Animation".to_string(),
            width: 1280,
            height: 720,
        }))
        .add_plugin(SkinnedBatchingPlugin)
        .add_system(SystemStage::PreUpdate, setup)
        .add_system(SystemStage::Update, advance_animation)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut skinned_meshes: ResMut<Assets<SkinnedMesh>>,
    mut materials: ResMut<Assets<Material>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let mut loaded = SkinnedMeshBuilder::from_file("../assets/models/simple_character.gltf")
        .build(&mut skinned_meshes)
        .ok()?;

    let mesh_handle = loaded.mesh()?;

    // Play the first available animation clip, if any.
    let clip_name = loaded.player.clip_names().next().map(|s| s.to_string());
    if let Some(name) = clip_name {
        loaded.player.play(&name);
    }

    let material = MaterialBuilder::new(SHADER)
        .with_label("skinned")
        .with_vertex_layouts(vec![SkinnedVertex::layout()])
        .with_entries(vec![GroupEntry::Global("pebble_skinning")])
        .without_cull_mode()
        .with_targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: None,
            write_mask: ColorWrites::ALL,
        }])
        .build_asset("skinned", &mut materials);

    commands.spawn((mesh_handle, material, loaded.player));
    Some(())
}

/// Advances every animated entity's clock. The `SkinnedBatchingPlugin` reads
/// each entity's `AnimationPlayer` every `PreRender` tick — this system just
/// needs to keep time moving.
fn advance_animation(time: Res<Time>, mut query: Query<&mut AnimationPlayer>) {
    for player in query.iter() {
        player.advance(time.delta_seconds());
    }
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<Assets<Material>>,
    meshes: Res<Assets<SkinnedMesh>>,
    renderer: Option<Res<SkinnedBatchRenderer>>,
    storage: Option<Res<SkinnedBatchStorage>>,
) {
    let Some(renderer) = renderer else { return };
    let Some(storage) = storage else { return };
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for batch in storage.batches.iter() {
        let Some(mesh) = meshes.get(Handle::<SkinnedMesh>::new(batch.mesh)) else { continue };
        let Some(material) = materials.get(Handle::<Material>::new(batch.material)) else { continue };
        let Some(bind_group) = renderer.bind_group(batch.material, batch.mesh) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..batch.instance_count);
    }
}
