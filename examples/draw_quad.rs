use pebble::prelude::*;
use std::sync::Arc;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

struct WindowConfig {
    name: &'static str,
    width: u32,
    height: u32,
}

struct WindowResource {
    window: Arc<Window>,
}

struct WindowPlugin {
    config: WindowConfig,
}

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        let name = self.config.name;
        let width = self.config.width;
        let height = self.config.height;

        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            WindowBuilder::default()
                .with_title(name)
                .with_inner_size(PhysicalSize::new(width, height))
                .build(&event_loop)
                .unwrap(),
        );

        app.add_resource(WindowResource {
            window: window.clone(),
        });

        app.set_runner(move |mut app| {
            event_loop
                .run(move |event, elwt| match event {
                    Event::Resumed => {}
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                        }
                        _ => {}
                    },
                    Event::AboutToWait => {
                        window.request_redraw();
                    }
                    _ => {}
                })
                .unwrap();
        });
    }
}

struct GPUResource {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(SystemStage::Startup, setup_ctx)
            .add_system(SystemStage::Render, render);
    }
}

fn setup_ctx(mut commands: Commands, window: Res<WindowResource>) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance.create_surface(window.window.clone()).unwrap();

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

    let caps = surface.get_capabilities(&adapter);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        width: 1920,
        height: 1080,
        format: caps.formats[0],
        present_mode: caps.present_modes[0],
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    commands.insert_resource(GPUResource {
        device,
        queue,
        surface,
        config,
    });
}

fn render(ctx: Option<Res<GPUResource>>) {
    if let Some(ctx) = ctx {
        let surface_texture = match ctx.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            _ => {
                return;
            }
        };
        let surface_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    } else {
        println!("Render Context not found?");
    }
}

struct Vertex {
    pos: [f32; 3],
}

struct GPUMesh {
    vert_buf: wgpu::Buffer,
    idx_buf: wgpu::Buffer,
}

struct Mesh {
    vert: Vec<Vertex>,
    idx: Vec<u32>,
}

struct QuadMesh;
impl Asset for QuadMesh {
    type Intermediate = Mesh;

    fn source(&self) -> Mesh {
        Mesh {
            vert: vec![
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
            idx: vec![0, 1, 3, 3, 1, 2],
        }
    }
}

#[derive(Clone)]
struct MeshLoader;
impl AssetLoader for MeshLoader {
    type Asset = Mesh;
    type GPUCtx = GPUResource;
    type GPUOutput = GPUMesh;

    // sync cpu to gpu resources
    fn sync(&self, mut asset: &mut Self::Asset, gpu: &Self::GPUCtx) -> Self::GPUOutput {
        let vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            usage: wgpu::BufferUsages::VERTEX,
            size: (std::mem::size_of::<Vertex>() * asset.vert.len()) as wgpu::BufferAddress,
            mapped_at_creation: false,
        });

        let index_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            usage: wgpu::BufferUsages::INDEX,
            size: (std::mem::size_of::<u32>() * asset.idx.len()) as wgpu::BufferAddress,
            mapped_at_creation: false,
        });

        // clear the vec for idx, and vert
        asset.vert = Vec::new();
        asset.idx = Vec::new();

        GPUMesh {
            vert_buf: vertex_buffer,
            idx_buf: index_buffer,
        }
    }
}

fn main() {
    App::new()
        .add_plugin(WindowPlugin {
            config: WindowConfig {
                name: "Draw Quad Example",
                width: 1920,
                height: 1080,
            },
        })
        .add_plugin(RenderPlugin {})
        .add_plugin(AssetPlugin::new(MeshLoader {}))
        .add_system(SystemStage::Startup, setup_scene)
        .build()
        .run();
}

fn setup_scene(mut mesh_assets: ResMut<Assets<Mesh>>) {
    let quad_handle = mesh_assets.insert("Quad", QuadMesh {});
}
