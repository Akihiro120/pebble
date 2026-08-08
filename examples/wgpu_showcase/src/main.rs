use pebble::prelude::*;
use pebble::wgpu::{
    backend::{WGPUBackend, WGPUPlugin},
    binding::{BindingEntry, BindingKind},
    flags::ShaderStages,
    instance::{GPUMaterialInstance, MaterialInstance, MaterialInstanceBuilder},
    layout::GroupEntry,
    material::{ColorTargetState, GPUMaterial, Material, MaterialBuilder},
    mesh::{GPUMesh, Mesh, MeshBuilder, Vertex},
    render_pass::IndexFormat,
    samplers::SamplerKind,
    textures::{Texture, TextureBuilder},
};

const SHADER: &str = r#"
struct VOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var out: VOut;
    out.clip_pos = vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var albedo: texture_2d<f32>;
@group(0) @binding(1) var albedo_sampler: sampler;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(albedo, albedo_sampler, in.uv);
}
"#;

// `mesh::Vertex` carries position/tex_coords/normal/tangent — the shader
// above only reads locations 0 (position) and 1 (tex_coords), so normal and
// tangent are set but unused here. A flat quad facing the camera.
fn quad_vertices() -> Vec<Vertex> {
    let normal = glam::Vec3::Z;
    let tangent = glam::Vec4::new(1.0, 0.0, 0.0, 1.0);
    vec![
        Vertex::new(glam::Vec3::new(-0.6, 0.6, 0.0), glam::Vec2::new(0.0, 0.0), normal, tangent),
        Vertex::new(glam::Vec3::new(-0.6, -0.6, 0.0), glam::Vec2::new(0.0, 1.0), normal, tangent),
        Vertex::new(glam::Vec3::new(0.6, -0.6, 0.0), glam::Vec2::new(1.0, 1.0), normal, tangent),
        Vertex::new(glam::Vec3::new(0.6, 0.6, 0.0), glam::Vec2::new(1.0, 0.0), normal, tangent),
    ]
}
const INDICES: [u32; 6] = [0, 1, 2, 2, 3, 0];

fn material_entries() -> Vec<BindingEntry> {
    vec![
        BindingEntry {
            name: "albedo",
            binding: 0,
            kind: BindingKind::texture_2d(ShaderStages::FRAGMENT),
        },
        BindingEntry {
            name: "albedo_sampler",
            binding: 1,
            kind: BindingKind::sampler(ShaderStages::FRAGMENT),
        },
    ]
}

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "WGPU Module Showcase".to_string(),
            width: 1280,
            height: 720,
        }))
        .add_system(SystemStage::PreUpdate, setup.once())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut textures: ResMut<Assets<Texture>>,
    mut instances: ResMut<Assets<MaterialInstance>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let quad = MeshBuilder::new(quad_vertices(), INDICES.to_vec()).build_asset("quad", &mut meshes);

    let brick = TextureBuilder::from_file("../assets/textures/brick.png")
        .with_mips()
        .build_asset("brick", &mut textures);

    let material = MaterialBuilder::new(SHADER)
        .label("quad-material")
        .vertex_entry("vs_main")
        .fragment_entry("fs_main")
        .vertex_layouts(vec![Vertex::layout()])
        .entries(vec![GroupEntry::Own(material_entries())])
        .targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: None,
            write_mask: Default::default(),
        }])
        .build_asset("quad_material", &mut materials);

    let brick_instance = MaterialInstanceBuilder::new(material)
        .texture("albedo", brick)
        .sampler("albedo_sampler", SamplerKind::LinearRepeat)
        .build_asset("brick_instance", &mut instances);

    commands.spawn((quad, brick_instance));

    Some(())
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) {
    let Some(mut active) = frame.active() else {
        return;
    };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for (mesh_handle, instance_handle) in query.iter() {
        let Some(mesh) = meshes.get(mesh_handle.id) else {
            continue;
        };
        let Some(instance) = instances.get(instance_handle.id) else {
            continue;
        };
        let Some(material) = materials.get(instance.target) else {
            continue;
        };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, &instance.bind_group, &[]);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
