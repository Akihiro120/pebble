mod common;

use common::winit_window::WinitWindow;
use pebble::prelude::*;

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
    type Context = wgpu::CommandEncoder;

    fn context(&mut self) -> &mut Self::Context {
        &mut self.encoder
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
            width,
            height,
            format: caps.formats[0],
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        WGPUBackend {
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
            _ => return None,
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

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            name: "Draw Quad Example",
            width: 1920,
            height: 1080,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(frame) = &mut frame.frame {
        let mut _pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                multiview_mask: None,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
    }
}
