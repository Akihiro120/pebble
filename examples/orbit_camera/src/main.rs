use std::time::Instant;

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
    normals: [f32; 3],
}

struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload<'a>(
        source: &Self::Source,
        backend: &WGPUBackend,
        _deps: &Self::Deps<'a>,
    ) -> Option<Self> {
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
    vertex: &'static [u8],
    fragment: &'static [u8],
}

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = (Res<'a, Camera>, Res<'a, DepthTexture>);

    fn upload<'a>(
        source: &Self::Source,
        backend: &WGPUBackend,
        deps: &Self::Deps<'a>,
    ) -> Option<Self> {
        let (camera, depth) = deps;

        let vertex_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::util::make_spirv(source.vertex),
            });

        let fragment_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::util::make_spirv(source.fragment),
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
                    bind_group_layouts: &[
                        Some(&camera.bind_group_layout),
                        Some(&bind_group_layout),
                    ],
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
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                                shader_location: 2,
                            },
                        ],
                    }],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth.texture.format(),
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
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

    fn upload<'a>(
        source: &Self::Source,
        backend: &WGPUBackend,
        _deps: &Self::Deps<'a>,
    ) -> Option<Self> {
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

    fn upload<'a>(
        source: &Self::Source,
        backend: &WGPUBackend,
        deps: &Self::Deps<'a>,
    ) -> Option<Self> {
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

#[derive(Copy, Clone, Default)]
struct TransformComponent {
    position: glam::Vec3,
    rotation: glam::Vec3,
    scale: glam::Vec3,
}

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    proj: glam::Mat4,
    view: glam::Mat4,
}

struct Camera {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}

struct CameraComponent {
    uniform: CameraUniform,
}

struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            SystemStage::Update,
            (create_camera, update_camera, update_camera_buffer),
        );
    }
}

fn create_camera(
    mut commands: Commands,
    backend: Option<Res<WGPUBackend>>,
    camera: Option<Res<Camera>>,
) {
    if let None = camera
        && let Some(backend) = backend
    {
        let buffer = backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout =
            backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        commands.insert_resource(Camera {
            bind_group_layout,
            bind_group,
            buffer,
        });
    }
}

fn update_camera(
    window: Res<WindowResource<WinitWindow>>,
    time: Res<Time>,
    mut query: Query<(&mut CameraComponent, &TransformComponent)>,
) {
    let size = window.handle.inner_size();
    let aspect_ratio = size.width as f32 / size.height as f32;

    for (active_camera, _transform) in query.iter() {
        active_camera.uniform.proj =
            glam::Mat4::perspective_rh(45.0f32.to_radians(), aspect_ratio, 0.1, 100.0);

        let radius = 5.0;
        let cam_x = f32::sin(time.time.elapsed().as_secs_f32()) * radius;
        let cam_y = f32::sin(time.time.elapsed().as_secs_f32()) * radius;
        let cam_z = f32::cos(time.time.elapsed().as_secs_f32()) * radius;
        active_camera.uniform.view = glam::Mat4::look_at_rh(
            glam::Vec3::new(cam_x, cam_y, cam_z),
            glam::Vec3::default(),
            glam::Vec3::Y,
        );
    }
}

fn update_camera_buffer(
    backend: Option<Res<WGPUBackend>>,
    camera: Option<Res<Camera>>,
    mut query: Query<&CameraComponent>,
) {
    if let Some(backend) = backend
        && let Some(camera) = camera
    {
        for active_camera in query.iter() {
            backend.queue.write_buffer(
                &camera.buffer,
                0,
                bytemuck::bytes_of(&active_camera.uniform),
            );
        }
    }
}

struct DepthTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct DepthPlugin;
impl Plugin for DepthPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(SystemStage::PreRender, create_depth);
    }
}

fn create_depth(
    mut commands: Commands,
    backend: Option<Res<WGPUBackend>>,
    depth: Option<Res<DepthTexture>>,
) {
    if let Some(backend) = backend
        && let None = depth
    {
        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: backend.config.width,
                height: backend.config.height,
                depth_or_array_layers: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
            format: wgpu::TextureFormat::Depth16Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        commands.insert_resource(DepthTexture { texture, view });
    }
}

struct Time {
    time: Instant,
    last_time: Instant,
    delta_time: f32,
}

struct TimePlugin;
impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Time {
            time: Instant::now(),
            last_time: Instant::now(),
            delta_time: 0.0,
        })
        .add_system(SystemStage::PreUpdate, update_delta_time);
    }
}

fn update_delta_time(mut time: ResMut<Time>) {
    let current_time = Instant::now();
    time.delta_time = (current_time - time.last_time).as_secs_f32();
    time.last_time = current_time;
}

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            title: "Orbiting Camera",
            width: 1920,
            height: 1080,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMaterial>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUTexture>::new())
        .add_plugin(AssetPlugin::<WGPUBackend, GPUMaterialInstance>::new())
        .add_plugin(TimePlugin)
        .add_plugin(DepthPlugin)
        .add_plugin(CameraPlugin)
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
    let cube_mesh = meshes.insert(
        "quad",
        Mesh {
            vertices: vec![
                Vertex {
                    pos: [-0.5, 0.5, 0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [0.0, 0.0, 1.0],
                }, // 0
                Vertex {
                    pos: [-0.5, -0.5, 0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [0.0, 0.0, 1.0],
                }, // 1
                Vertex {
                    pos: [0.5, -0.5, 0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [0.0, 0.0, 1.0],
                }, // 2
                Vertex {
                    pos: [0.5, 0.5, 0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [0.0, 0.0, 1.0],
                }, // 3
                Vertex {
                    pos: [0.5, 0.5, -0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [0.0, 0.0, -1.0],
                }, // 4
                Vertex {
                    pos: [0.5, -0.5, -0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [0.0, 0.0, -1.0],
                }, // 5
                Vertex {
                    pos: [-0.5, -0.5, -0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [0.0, 0.0, -1.0],
                }, // 6
                Vertex {
                    pos: [-0.5, 0.5, -0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [0.0, 0.0, -1.0],
                }, // 7
                Vertex {
                    pos: [-0.5, 0.5, -0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [0.0, 1.0, 0.0],
                }, // 8
                Vertex {
                    pos: [-0.5, 0.5, 0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [0.0, 1.0, 0.0],
                }, // 9
                Vertex {
                    pos: [0.5, 0.5, 0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [0.0, 1.0, 0.0],
                }, // 10
                Vertex {
                    pos: [0.5, 0.5, -0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [0.0, 1.0, 0.0],
                }, // 11
                Vertex {
                    pos: [-0.5, -0.5, 0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [0.0, -1.0, 0.0],
                }, // 12
                Vertex {
                    pos: [-0.5, -0.5, -0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [0.0, -1.0, 0.0],
                }, // 13
                Vertex {
                    pos: [0.5, -0.5, -0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [0.0, -1.0, 0.0],
                }, // 14
                Vertex {
                    pos: [0.5, -0.5, 0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [0.0, -1.0, 0.0],
                }, // 15
                Vertex {
                    pos: [0.5, 0.5, 0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [1.0, 0.0, 0.0],
                }, // 16
                Vertex {
                    pos: [0.5, -0.5, 0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [1.0, 0.0, 0.0],
                }, // 17
                Vertex {
                    pos: [0.5, -0.5, -0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [1.0, 0.0, 0.0],
                }, // 18
                Vertex {
                    pos: [0.5, 0.5, -0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [1.0, 0.0, 0.0],
                }, // 19
                Vertex {
                    pos: [-0.5, 0.5, -0.5],
                    tex_coords: [0.0, 0.0],
                    normals: [-1.0, 0.0, 0.0],
                }, // 20
                Vertex {
                    pos: [-0.5, -0.5, -0.5],
                    tex_coords: [0.0, 1.0],
                    normals: [-1.0, 0.0, 0.0],
                }, // 21
                Vertex {
                    pos: [-0.5, -0.5, 0.5],
                    tex_coords: [1.0, 1.0],
                    normals: [-1.0, 0.0, 0.0],
                }, // 22
                Vertex {
                    pos: [-0.5, 0.5, 0.5],
                    tex_coords: [1.0, 0.0],
                    normals: [-1.0, 0.0, 0.0],
                }, // 23
            ],
            indices: vec![
                0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15,
                16, 17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23,
            ],
        },
    );

    let lit_mat = materials.insert(
        "lit",
        Material {
            vertex: include_bytes!("../../assets/shaders/compiled/lit.vert.spv"),
            fragment: include_bytes!("../../assets/shaders/compiled/lit.frag.spv"),
        },
    );

    let brick_texture = textures.insert(
        "brick",
        Texture {
            path: "../assets/textures/brick.png",
        },
    );

    let lit_mat_inst = material_instances.insert(
        "quad_brick",
        MaterialInstance {
            base: lit_mat,
            albedo_id: brick_texture,
        },
    );

    commands.spawn((
        CameraComponent {
            uniform: CameraUniform::default(),
        },
        TransformComponent {
            position: glam::Vec3::new(0.0, 0.0, -3.0),
            ..Default::default()
        },
    ));

    commands.spawn((
        Handle::<Mesh>::new(cube_mesh),
        Handle::<MaterialInstance>::new(lit_mat_inst),
    ));
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    mat_inst: Res<ProcessedAssets<GPUMaterialInstance>>,
    camera: Option<Res<Camera>>,
    depth_texture: Option<Res<DepthTexture>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) {
    let Some(camera) = &camera else {
        return;
    };

    let Some(depth) = &depth_texture else {
        return;
    };

    if let Some(mut pass) = frame.begin_pass(Pass {
        colors: &[ColorTarget::Default {
            clear: Some([0.2, 0.3, 0.3, 1.0]),
        }],
        depth: Some(DepthTarget {
            attachment: &depth.view,
            clear: Some(1.0),
        }),
    }) {
        pass.set_bind_group(0, Some(&camera.bind_group), &[]);

        for (mesh_id, mat_id) in query.iter() {
            let Some(mesh) = meshes.get(mesh_id.id) else {
                return;
            };

            let Some(inst) = mat_inst.get(mat_id.id) else {
                return;
            };

            pass.set_pipeline(&inst.pipeline);
            pass.set_bind_group(1, Some(&inst.bind_group), &[]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
