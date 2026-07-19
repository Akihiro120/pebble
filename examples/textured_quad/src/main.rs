use examples_common::*;
use image::GenericImageView;
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
    tex_coords: [f32; 2],
}

struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, _deps: &Self::Deps<'a>) -> Option<Self> {
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
    bind_group_layout: wgpu::BindGroupLayout,
}

struct Material {
    vertex: &'static str,
    fragment: &'static str,
}

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, _deps: &Self::Deps<'a>) -> Option<Self> {
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

        let bind_group_layout =
            backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let pipeline_layout =
            backend
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });

        let pipeline = backend
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
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
                        ],
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

        Some(Self {
            pipeline,
            bind_group_layout,
        })
    }
}

struct GPUTexture {
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    view: wgpu::TextureView,
}

struct Texture {
    path: &'static str,
}

impl Asset<WGPUBackend> for GPUTexture {
    type Source = Texture;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, _deps: &Self::Deps<'a>) -> Option<Self> {
        let img = image::open(source.path).expect("failed to load image");
        let rgba = img.to_rgba8();
        let (width, height) = img.dimensions();

        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            format: wgpu::TextureFormat::Rgba8Unorm,
            dimension: wgpu::TextureDimension::D2,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            view_formats: &[],
        });

        backend.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                aspect: wgpu::TextureAspect::All,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height * 4),
            },
            extent,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = backend.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        Some(Self {
            texture,
            sampler,
            view,
        })
    }
}

struct GPUMaterialInstance {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

struct MaterialInstance {
    base: RawAssetHandle,
    albedo_id: RawAssetHandle,
}

impl Asset<WGPUBackend> for GPUMaterialInstance {
    type Source = MaterialInstance;
    type Deps<'a> = (
        Res<'a, ProcessedAssets<GPUMaterial>>,
        Res<'a, ProcessedAssets<GPUTexture>>,
    );

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'a>) -> Option<Self> {
        let (materials, textures) = deps;

        let base_mat = materials.get(source.base)?;
        let albedo_tex = textures.get(source.albedo_id)?;

        let bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &base_mat.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&albedo_tex.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&albedo_tex.sampler),
                    },
                ],
            });

        Some(Self {
            pipeline: base_mat.pipeline.clone(),
            bind_group: bind_group,
        })
    }
}

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            title: "Textured Quad",
            width: 1920,
            height: 1080,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMaterial>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUTexture>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMaterialInstance>::new())
        .add_system(SystemStage::Startup, setup)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut textures: ResMut<Assets<Texture>>,
    mut material_instances: ResMut<Assets<MaterialInstance>>,
) {
    let quad_mesh = meshes.insert(
        "quad",
        Mesh {
            vertices: vec![
                Vertex {
                    pos: [-0.5, 0.5, 0.0],
                    tex_coords: [1.0, 0.0],
                },
                Vertex {
                    pos: [-0.5, -0.5, 0.0],
                    tex_coords: [1.0, 1.0],
                },
                Vertex {
                    pos: [0.5, -0.5, 0.0],
                    tex_coords: [0.0, 1.0],
                },
                Vertex {
                    pos: [0.5, 0.5, 0.0],
                    tex_coords: [0.0, 0.0],
                },
            ],
            indices: vec![0, 1, 3, 3, 1, 2],
        },
    );

    let quad_mat = materials.insert(
        "quad",
        Material {
            vertex: "../assets/shaders/compiled/texture_quad.vert.spv",
            fragment: "../assets/shaders/compiled/texture_quad.frag.spv",
        },
    );

    let brick_texture = textures.insert(
        "brick",
        Texture {
            path: "../assets/textures/brick.png",
        },
    );

    let quad_mat_inst = material_instances.insert(
        "quad_brick",
        MaterialInstance {
            base: quad_mat,
            albedo_id: brick_texture,
        },
    );

    commands.spawn((
        Handle::<Mesh>::new(quad_mesh),
        Handle::<MaterialInstance>::new(quad_mat_inst),
    ));
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    mat_inst: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) {
    if let Some(mut active) = frame.active() {
        let mut pass = active.render_context([0.2, 0.3, 0.3, 1.0]);
        for (mesh_id, mat_id) in query.iter() {
            let Some(mesh) = meshes.get(mesh_id.id) else {
                return;
            };

            let Some(inst) = mat_inst.get(mat_id.id) else {
                return;
            };

            pass.set_pipeline(&inst.pipeline);
            pass.set_bind_group(0, Some(&inst.bind_group), &[]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
