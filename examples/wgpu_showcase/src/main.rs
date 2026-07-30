use pebble::prelude::*;
use pebble::wgpu::{
    backend::{WGPUBackend, WGPUPlugin},
    material::{GPUMaterial, MaterialBindingEntry, MaterialBindingKind, MaterialDescriptor},
    material_instance::{
        GPUMaterialInstance, MaterialInstanceBindingEntry, MaterialInstanceDescriptor,
    },
    mesh::{GPUMesh, Mesh},
    samplers::SamplerKind,
    textures::TextureSpec,
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

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    uv: [f32; 2],
}

const VERTICES: [Vertex; 4] = [
    Vertex {
        pos: [-0.6, 0.6, 0.0],
        uv: [0.0, 0.0],
    },
    Vertex {
        pos: [-0.6, -0.6, 0.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        pos: [0.6, -0.6, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        pos: [0.6, 0.6, 0.0],
        uv: [1.0, 0.0],
    },
];
const INDICES: [u32; 6] = [0, 1, 2, 2, 3, 0];

const VERTEX_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
        shader_location: 1,
    },
];

const VERTEX_LAYOUT: [wgpu::VertexBufferLayout; 1] = [wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &VERTEX_ATTRS,
}];

const MATERIAL_ENTRIES: [MaterialBindingEntry; 2] = [
    MaterialBindingEntry {
        name: "albedo",
        kind: MaterialBindingKind::Texture,
    },
    MaterialBindingEntry {
        name: "albedo_sampler",
        kind: MaterialBindingKind::Sampler,
    },
];

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "WGPU Module Showcase",
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
    mut materials: ResMut<Assets<MaterialDescriptor<'static>>>,
    mut textures: ResMut<Assets<TextureSpec>>,
    mut instances: ResMut<Assets<MaterialInstanceDescriptor>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let quad = meshes.insert(
        "quad",
        Mesh {
            vertices: bytemuck::cast_slice(&VERTICES).to_vec(),
            indices: INDICES.to_vec(),
        },
    );

    let brick = textures.insert(
        "brick",
        TextureSpec {
            file: Some("../assets/textures/brick.png"),
            width: 0,
            height: 0,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            data: None,
            generate_mips: true,
        },
    );

    let material = materials.insert(
        "quad_material",
        MaterialDescriptor {
            label: Some("quad-material"),
            shader_source: SHADER,
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: VERTEX_LAYOUT.to_vec(),
            entries: MATERIAL_ENTRIES.to_vec(),
            targets: vec![wgpu::ColorTargetState {
                format: backend.config.format,
                blend: None,
                write_mask: Default::default(),
            }],
            ..Default::default()
        },
    );

    let brick_instance = instances.insert(
        "brick_instance",
        MaterialInstanceDescriptor {
            material,
            params: vec![
                ("albedo", MaterialInstanceBindingEntry::Texture(brick)),
                (
                    "albedo_sampler",
                    MaterialInstanceBindingEntry::Sampler(SamplerKind::LinearRepeat),
                ),
            ],
        },
    );

    commands.spawn((
        Handle::<Mesh>::new(quad),
        Handle::<MaterialInstanceDescriptor>::new(brick_instance),
    ));

    Some(())
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstanceDescriptor>)>,
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
        let Some(material) = materials.get(instance.material) else {
            continue;
        };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, Some(&instance.bind_group), &[]);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
