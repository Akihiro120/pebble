use std::sync::Arc;

use pebble::prelude::*;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub struct WinitWindow {
    pub window: Arc<Window>,
    pub event_loop: EventLoop<()>,
}

impl WindowProvider for WinitWindow {
    type Handle = Arc<Window>;
    type Exposed = ();

    fn create(config: &WindowConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();
        let window = Arc::new(
            WindowBuilder::default()
                .with_title(config.title)
                .with_inner_size(PhysicalSize::new(config.width, config.height))
                .build(&event_loop)
                .unwrap(),
        );

        Self { window, event_loop }
    }

    fn size(handle: &Self::Handle) -> (u32, u32) {
        let s = handle.inner_size();
        (s.width, s.height)
    }

    fn exposed(&self) -> Self::Exposed {}

    fn handle(&self) -> &Self::Handle {
        &self.window
    }
}

impl WindowRunner for WinitWindow {
    fn run(self, mut on_frame: impl FnMut() + 'static) {
        self.event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::RedrawRequested => {
                        on_frame();
                    }
                    _ => {}
                },
                Event::AboutToWait => self.window.request_redraw(),
                _ => {}
            })
            .unwrap();
    }
}

impl PresentableWindow for WinitWindow {}

pub struct WGPUBackend {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

pub struct WGPUFrame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface_texture: wgpu::SurfaceTexture,
}

impl FrameOperations for WGPUFrame {
    type Context<'a> = wgpu::RenderPass<'a>;
    type Attachment = wgpu::TextureView;
    type DepthAttachment = wgpu::TextureView;

    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_> {
        let color_attachments: Vec<_> = pass
            .colors
            .iter()
            .map(|target| {
                let (view, clear) = match target {
                    ColorTarget::Default { clear } => (&self.view, *clear),
                    ColorTarget::Custom { attachment, clear } => (*attachment, *clear),
                };

                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: clear
                            .map(|[r, g, b, a]| {
                                wgpu::LoadOp::Clear(wgpu::Color {
                                    r: r as f64,
                                    g: g as f64,
                                    b: b as f64,
                                    a: a as f64,
                                })
                            })
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();

        let depth_stencil_attachment = pass.depth.map(|d| wgpu::RenderPassDepthStencilAttachment {
            view: d.attachment,
            depth_ops: Some(wgpu::Operations {
                load: d
                    .clear
                    .map(wgpu::LoadOp::Clear)
                    .unwrap_or(wgpu::LoadOp::Load),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        });

        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

impl Backend for WGPUBackend {
    type Frame = WGPUFrame;

    fn init(
        handle: impl GPUSurfaceHandle,
        width: u32,
        height: u32,
        sender: pebble::rendering::sync::InitSender<Self>,
    ) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            display: None,
            backends: wgpu::Backends::default(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
        });
        let surface = instance.create_surface(handle).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::default(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .unwrap_or(&caps.formats[0])
            .clone();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: format,
            present_mode: caps.present_modes[0],
            alpha_mode: caps.alpha_modes[0],
            width,
            height,
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        sender.send(WGPUBackend {
            device,
            queue,
            surface,
            config,
        });
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost => {
                return Err(AcquireError::Transient);
            }
            other => {
                return Err(AcquireError::Fatal(format!(
                    "unexpected surface state: {other:?}"
                )));
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        Ok(Self::Frame {
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
