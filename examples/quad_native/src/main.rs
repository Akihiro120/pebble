use std::sync::Arc;

use pebble::prelude::*;
use wgpu::util::DeviceExt;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

struct WinitWindow {
    window: Arc<Window>,
    event_loop: EventLoop<()>,
}

impl PresentableWindow for WinitWindow {}

impl WindowProvider for WinitWindow {
    type Handle = Arc<Window>;
    type Exposed = ();

    fn create(config: &WindowConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();

        let window_builder = WindowBuilder::new()
            .with_title(config.title)
            .with_inner_size(PhysicalSize::new(config.width, config.height));

        let window = Arc::new(window_builder.build(&event_loop).unwrap());

        Self { window, event_loop }
    }

    fn size(handle: &Self::Handle) -> (u32, u32) {
        let s = handle.inner_size();
        (s.width, s.height)
    }

    fn exposed(&self) -> &Self::Exposed {
        &()
    }

    fn handle(&self) -> &Self::Handle {
        &self.window
    }
}

impl WindowRunner for WinitWindow {
    fn run(self, mut on_frame: impl FnMut() + 'static) {
        self.event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        elwt.exit();
                    }
                    WindowEvent::RedrawRequested => on_frame(),
                    _ => {}
                },
                Event::AboutToWait => {
                    self.window.request_redraw();
                }
                _ => {}
            })
            .unwrap();
    }
}

struct WGPUBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct WGPUFrame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface_texture: wgpu::SurfaceTexture,
}

impl FrameOperations for WGPUFrame {
    type Context<'a> = wgpu::RenderPass<'a>;

    fn context(&mut self) -> Self::Context<'_> {
        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

impl Backend for WGPUBackend {
    type Frame = WGPUFrame;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(handle).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            width: 1280,
            height: 720,
            format: caps.formats[0],
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            device,
            queue,
            surface,
            config,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn acquire(&mut self) -> Option<Self::Frame> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            _ => {
                return None;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        Some(WGPUFrame {
            encoder,
            view,
            surface_texture,
        })
    }

    fn present(&mut self, frame: Self::Frame) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.surface_texture.present();
    }
}

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

impl Drawable<WGPUBackend> for GPUMesh {
    fn draw(&self, pass: &mut wgpu::RenderPass) {
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

impl DeviceUpload<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'_>) -> Self {
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

        Self {
            vertex_buffer,
            index_buffer,
            index_count,
        }
    }
}

struct GPUMaterial {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

struct Material {
    vertex_path: &'static [u8],
    fragment_path: &'static [u8],
}

impl Bindable<WGPUBackend> for GPUMaterial {
    fn bind(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
    }
}

impl DeviceUpload<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = ();

    fn upload(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'_>) -> Self {
        // let vertex_data = std::fs::read(source.vertex_path).unwrap();
        // let fragment_data = std::fs::read(source.fragment_path).unwrap();

        let vertex_source = wgpu::util::make_spirv(&source.vertex_path);
        let fragment_source = wgpu::util::make_spirv(&source.fragment_path);

        let vertex_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: vertex_source,
            });

        let fragment_module = backend
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: fragment_source,
            });

        let bind_group_layout =
            backend
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
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
                        array_stride: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
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
                multiview_mask: None,
                cache: None,
            });

        Self {
            pipeline,
            layout: bind_group_layout,
        }
    }
}

struct GPUMaterialInstance {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadUniform {
    tint: [f32; 4],
}

struct MaterialInstance {
    handle: RawAssetHandle,
    uniform: QuadUniform,
}

impl Bindable<WGPUBackend> for GPUMaterialInstance {
    fn bind(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(&self.bind_group), &[]);
    }
}

impl DeviceUpload<WGPUBackend> for GPUMaterialInstance {
    type Source = MaterialInstance;
    type Deps<'a> = Res<'a, GPUMaterial>;

    fn upload(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'_>) -> Self {
        let material = deps;

        let buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&source.uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group = backend
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &material.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        Self {
            pipeline: material.pipeline.clone(),
            bind_group,
        }
    }
}

fn main() {
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            title: "Quad Native Example",
            width: 1280,
            height: 720,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_plugin(DeviceAssetPlugin::<WGPUBackend, GPUMesh>::new())
        .add_plugin(DeviceAssetPlugin::<WGPUBackend, GPUMaterial>::new())
        .add_plugin(DeviceAssetPlugin::<WGPUBackend, GPUMaterialInstance>::new())
        .add_system(SystemStage::Startup, setup)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut command: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut material_instances: ResMut<Assets<MaterialInstance>>,
) {
    let quad_mesh = meshes.insert(
        "quad",
        Mesh {
            vertices: vec![
                Vertex {
                    pos: [-0.5, 0.5, 0.0],
                },
                Vertex {
                    pos: [-0.5, -0.5, 0.0],
                },
                Vertex {
                    pos: [0.5, -0.5, 0.0],
                },
                Vertex {
                    pos: [0.5, 0.5, 0.0],
                },
            ],
            indices: vec![0, 1, 3, 3, 1, 2],
        },
    );

    let quad_mat = materials.insert(
        "quad",
        Material {
            vertex_path: include_bytes!("../shaders/compiled/quad.vert.spv"),
            fragment_path: include_bytes!("../shaders/compiled/quad.frag.spv"),
        },
    );

    let quad_mat_red = material_instances.insert(
        "quad",
        MaterialInstance {
            handle: quad_mat,
            uniform: QuadUniform {
                tint: [1.0, 0.0, 0.0, 1.0],
            },
        },
    );

    command.spawn((
        Handle::<Mesh>::new(quad_mesh),
        Handle::<MaterialInstance>::new(quad_mat_red),
    ));
}

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    meshes: Res<GPUAssets<GPUMesh>>,
    material_instance: Res<GPUAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
) {
    if let Some(mut frame) = frame.get_render_context() {
        for (mesh_id, material_id) in query.iter() {
            let mesh = meshes.get(mesh_id.id).unwrap();
            let mat = material_instance.get(material_id.id).unwrap();

            mat.bind(&mut frame);
            mesh.draw(&mut frame);
        }
    }
}
