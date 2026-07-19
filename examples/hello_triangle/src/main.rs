use examples_common::*;
use pebble::prelude::*;
use wgpu::util::DeviceExt;

struct GPUMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
}

struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'a>) -> Option<Self> {
        let vertex_buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&source.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&source.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let index_count = source.indices.len() as u32;

        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count,
        })
    }
}

struct GPUMaterial {
    pipeline: wgpu::RenderPipeline,
}

struct Material {
    vertex: &'static str,
    fragment: &'static str,
}

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'a>) -> Option<Self> {
        let vert_bytes = std::fs::read(source.vertex).expect("failed to read vertex shader");
        let frag_bytes = std::fs::read(source.fragment).expect("failed to read fragment shader");

        let vertex_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::util::make_spirv(&vert_bytes),
            });

        let fragment_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::util::make_spirv(&frag_bytes),
            });

        let pipeline = backend
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multiview_mask: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: backend.config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                cache: None,
            });

        Some(Self { pipeline })
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            title: "Hello Triangle",
            width: 1920,
            height: 1080,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMaterial>::new())
        .add_system(SystemStage::Startup, setup)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
) {
    let triangle_mesh = meshes.insert(
        "triangle",
        Mesh {
            vertices: vec![
                Vertex {
                    pos: [0.0, 0.5, 0.0],
                },
                Vertex {
                    pos: [-0.5, -0.5, 0.0],
                },
                Vertex {
                    pos: [0.5, -0.5, 0.0],
                },
            ],
            indices: vec![0, 1, 2],
        },
    );

    let triangle_mat = materials.insert(
        "triangle",
        Material {
            vertex: "../assets/shaders/compiled/quad.vert.spv",
            fragment: "../assets/shaders/compiled/quad.frag.spv",
        },
    );

    commands.spawn((
        Handle::<Mesh>::new(triangle_mesh),
        Handle::<Material>::new(triangle_mat),
    ));
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    mut query: Query<(&Handle<Mesh>, &Handle<Material>)>,
) {
    if let Some(mut active) = frame.active() {
        let mut pass = active.render_context([0.2, 0.3, 0.3, 1.0]);
        for (mesh_id, mat_id) in query.iter() {
            let Some(mesh) = meshes.get(mesh_id.id) else {
                return;
            };

            let Some(mat) = materials.get(mat_id.id) else {
                return;
            };

            pass.set_pipeline(&mat.pipeline);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
