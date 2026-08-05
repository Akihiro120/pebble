//! A textured quad, same as `textured_quad`, but rendered with MSAA turned
//! on and recorded once into a render bundle instead of issuing the same
//! draw calls by hand every frame — see the book's "Multisampled
//! Anti-Aliasing (MSAA)" and "Render Bundles" pages for the concepts.

use pebble::prelude::*;
use pebble::wgpu::prelude::*;
use pebble::wgpu::{
    backend::{WGPUBackend, WGPUPlugin},
    instance::{BindingInstanceEntry, GPUMaterialInstance, MaterialInstance},
    material::{GPUMaterial, Material},
    mesh::{GPUMesh, Mesh, Vertex},
    textures::Texture,
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
        BindingEntry { name: "albedo", binding: 0, kind: BindingKind::texture_2d(ShaderStages::FRAGMENT) },
        BindingEntry { name: "albedo_sampler", binding: 1, kind: BindingKind::sampler(ShaderStages::FRAGMENT) },
    ]
}

/// The recorded, replayable draw — built once, executed every frame.
struct QuadBundle(RenderBundle);

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Advanced Rendering (MSAA + Render Bundles)".to_string(),
            width: 1280,
            height: 720,
        }))
        .add_system(SystemStage::PreUpdate, setup_msaa.once())
        .add_system(SystemStage::PreUpdate, setup.once())
        .add_system(SystemStage::PreUpdate, build_bundle.once())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

/// Must run before any material meant to render into the default target is
/// built, so `backend.sample_count()` is already set to what that material
/// picks up — see the book's MSAA page.
fn setup_msaa(mut backend: ResMut<WGPUBackend>) -> Option<()> {
    backend.set_msaa(4);
    Some(())
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut textures: ResMut<Assets<Texture>>,
    mut instances: ResMut<Assets<MaterialInstance>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let quad = Mesh::new(quad_vertices(), INDICES.to_vec()).build_asset("quad", &mut meshes);

    let brick = Texture::from_file("../assets/textures/brick.png").build_asset("brick", &mut textures);

    let material = Material::new(SHADER)
        .label("quad-material")
        .vertex_layouts(vec![Vertex::layout()])
        .entries(material_entries())
        .targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: None,
            write_mask: Default::default(),
        }])
        .sample_count(backend.sample_count()) // matches the MSAA target set_msaa configured above
        .build_asset("quad_material", &mut materials);

    let quad_instance = MaterialInstance::new(
        material.id,
        vec![
            ("albedo", BindingInstanceEntry::Texture(brick.id)),
            ("albedo_sampler", BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)),
        ],
    )
    .build_asset("quad_brick", &mut instances);

    commands.spawn((quad, quad_instance));
    Some(())
}

/// Records the quad's draw once into a `RenderBundle`, as soon as its mesh,
/// material, and instance have all finished uploading. Retried every tick
/// (via `.once()`) until that's true.
fn build_bundle(
    mut commands: Commands,
    backend: Res<WGPUBackend>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) -> Option<()> {
    let (mesh_handle, instance_handle) = query.iter().next()?;
    let mesh = meshes.get(mesh_handle.id)?;
    let instance = instances.get(instance_handle.id)?;
    let material = materials.get(instance.target)?;

    let mut encoder = RenderBundleEncoderBuilder::new()
        .label("quad-bundle-encoder")
        .color_formats(vec![Some(backend.surface_format())])
        .sample_count(backend.sample_count())
        .build(&backend);
    encoder.set_pipeline(&material.pipeline);
    encoder.set_bind_group(0, &instance.bind_group, &[]);
    encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
    encoder.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
    encoder.draw_indexed(0..mesh.index_count, 0, 0..1);
    let bundle = encoder.finish(Some("quad-bundle"));

    commands.insert_resource(QuadBundle(bundle));
    Some(())
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>, bundle: Option<Res<QuadBundle>>) {
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    // The bundle isn't ready on the very first few frames — clear the
    // screen (above) but skip the draw itself, same "not ready yet, not an
    // error" shape as every other asset lookup in this book.
    if let Some(bundle) = bundle {
        pass.execute_bundles(&[&bundle.0]);
    }
}
