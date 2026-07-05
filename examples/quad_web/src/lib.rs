use std::sync::Arc;

use pebble::{prelude::*, rendering::web_backend::AsyncInit};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
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

impl RenderTarget for WinitWindow {}

impl WindowProvider for WinitWindow {
    type Handle = Arc<Window>;

    fn create(config: &WindowConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();

        let window_builder = WindowBuilder::new()
            .with_title(config.name)
            .with_inner_size(PhysicalSize::new(config.width, config.height));

        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowBuilderExtWebSys;

        let window = wgpu::web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document.get_element_by_id("canvas").unwrap();
        let html_canvas_element = canvas.unchecked_into::<HtmlCanvasElement>();

        let window = Arc::new(
            window_builder
                .with_canvas(Some(html_canvas_element.clone()))
                .build(&event_loop)
                .unwrap(),
        );

        Self { window, event_loop }
    }

    fn size(handle: &Self::Handle) -> (u32, u32) {
        let s = handle.inner_size();
        (s.width, s.height)
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
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
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
        unreachable!("WGPUBackend on web is initialized via AsyncInit::init_async")
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

impl AsyncInit for WGPUBackend {
    fn init_async(
        handle: impl GPUSurfaceHandle,
        width: u32,
        height: u32,
        sender: pebble::rendering::sync::InitSender<Self>,
    ) {
        wasm_bindgen_futures::spawn_local(async move {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let surface = instance.create_surface(handle).unwrap();

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                })
                .await
                .unwrap();

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .unwrap();

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

            sender.send(WGPUBackend {
                device,
                queue,
                surface,
                config,
            });
        });
    }
}

#[wasm_bindgen(start)]
pub fn run() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            name: "Quad Web Example",
            width: 1280,
            height: 720,
        }))
        .add_plugin(AsyncGraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(frame) = frame.get_render_context() {}
}
