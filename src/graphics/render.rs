use std::sync::Arc;

use crate::{
    app::BackendReady,
    ecs::{
        commands::Commands,
        plugin::Plugin,
        promise::{Promise, PromiseState},
        resources::{Read, Write},
    },
    graphics::{
        render::frame::{CurrentFrame, Frame},
        types::TextureFormat,
        window::Window,
    },
};

pub mod compute_pass;
pub mod frame;
pub(crate) mod gpu_context;
pub mod render_pass;
pub mod targets;

pub struct Backend {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_configuration: wgpu::SurfaceConfiguration,
}

impl Backend {
    pub fn surface_width(&self) -> u32 {
        self.surface_configuration.width
    }

    pub fn surface_height(&self) -> u32 {
        self.surface_configuration.height
    }

    pub fn surface_format(&self) -> TextureFormat {
        self.surface_configuration.format.into()
    }

    // Records and submits a compute pass immediately, in its own command
    // encoder — deliberately not tied to the swapchain frame's lifecycle
    // (CurrentFrame/begin_frame/end_frame) the way a render pass is.
    // Compute work doesn't need a frame to exist at all; submitting right
    // away means whatever buffer it writes can be read back via
    // Buffer::read() as soon as the GPU actually finishes, not deferred
    // until some later render stage.
    pub fn dispatch_compute(&self, record: impl FnOnce(&mut compute_pass::ComputePass)) {
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let raw_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            let mut pass = compute_pass::ComputePass::new(raw_pass);
            record(&mut pass);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}

pub(crate) struct BackendPlugin;
impl Plugin for BackendPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.insert_resource(CurrentFrame::default())
            .add_gpu_system(crate::ecs::system::SystemStage::Update, obtain_gpu)
            .add_gpu_system(crate::ecs::system::SystemStage::Update, poll_gpu)
            .add_gpu_system(
                crate::ecs::system::SystemStage::Update,
                clean_up_gpu_acquisition_resources,
            )
            .add_system(crate::ecs::system::SystemStage::PreRender, begin_frame)
            .add_system(crate::ecs::system::SystemStage::PostRender, end_frame)
            .add_system(crate::ecs::system::SystemStage::PostRender, maintain_gpu)
    }
}

pub struct GPUReceiver {
    promise: Promise<Backend>,
}

async fn init_gpu(window: Arc<winit::window::Window>) -> Backend {
    let instance = wgpu::Instance::default();

    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    let required_features = wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER;
    #[cfg(target_arch = "wasm32")]
    let required_features = wgpu::Features::empty();

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features,
            ..Default::default()
        })
        .await
        .unwrap();

    let window_size = window.inner_size();

    let caps = surface.get_capabilities(&adapter);
    let surface_configuration = wgpu::SurfaceConfiguration {
        alpha_mode: caps.alpha_modes[0],
        present_mode: caps.present_modes[0],
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: caps.formats.iter().find(|f| !f.is_srgb()).unwrap().clone(),
        width: window_size.width,
        height: window_size.height,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_configuration);

    let backend = Backend {
        device,
        queue,
        surface,
        surface_configuration,
    };

    backend
}

pub(crate) fn obtain_gpu(
    mut commands: Commands,
    window: Read<Window>,
    receiver: Option<Read<GPUReceiver>>,
) {
    // already waiting on a previous request, nothing to do
    if receiver.is_some() {
        return;
    }

    let window = window.raw();
    let (fulfiller, promise) = Promise::new();

    let fut = async move {
        let result = init_gpu(window).await;
        fulfiller.fulfill(result);
    };

    // the window is bound to the main thread (winit panics if touched off
    // it), so gpu init runs synchronously here rather than on a spawned
    // thread — this only happens once, so blocking is fine
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(fut);
    }

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(fut);
    }

    commands.insert_resource(GPUReceiver { promise });
}

pub(crate) fn poll_gpu(
    mut commands: Commands,
    mut ready: Write<BackendReady>,
    receiver: Option<Read<GPUReceiver>>,
) {
    let Some(receiver) = receiver else {
        return;
    };

    if !ready.0 {
        // continue polling for the gpu
        match receiver.promise.poll() {
            PromiseState::Ready(backend) => {
                println!("GPU Ready adding as resource");
                commands.insert_resource(backend);
                ready.0 = true;
            }
            PromiseState::Pending => {}
            PromiseState::Disconnected => {
                tracing::error!(
                    "GPU backend init sender was dropped without ever sending a value — the \
                         app has no usable backend and will stay idle forever"
                );
            }
        }
    }
}

pub(crate) fn clean_up_gpu_acquisition_resources(
    mut commands: Commands,
    ready: Read<BackendReady>,
    receiver: Option<Read<GPUReceiver>>,
) {
    if ready.0 && receiver.is_some() {
        commands.remove_resource::<GPUReceiver>();
    }
}

pub(crate) fn begin_frame(
    mut backend: Write<Backend>,
    mut current_frame: Write<CurrentFrame>,
    window: Read<Window>,
) {
    let (width, height) = window.inner_size();
    if width > 0
        && height > 0
        && (width != backend.surface_configuration.width
            || height != backend.surface_configuration.height)
    {
        backend.surface_configuration.width = width;
        backend.surface_configuration.height = height;
        backend.surface.configure(&backend.device, &backend.surface_configuration);
    }

    let surface_texture = match backend.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture) => texture,
        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            backend.surface.configure(&backend.device, &backend.surface_configuration);
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            tracing::error!("surface validation error acquiring the next frame");
            return;
        }
    };

    let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let encoder = backend.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    current_frame.set(Frame::new(encoder, view, surface_texture));
}

// checks for finished GPU work (buffer map callbacks, etc.) once per tick —
// nothing else drives Device::poll, so anything relying on a callback
// (e.g. Buffer::read's Promise) only resolves because this runs
pub(crate) fn maintain_gpu(backend: Read<Backend>) {
    let _ = backend.device.poll(wgpu::PollType::Poll);
}

pub(crate) fn end_frame(backend: Read<Backend>, mut current_frame: Write<CurrentFrame>) {
    let Some(frame) = current_frame.take() else {
        return;
    };

    let (encoder, surface_texture) = frame.finish();
    backend.queue.submit(std::iter::once(encoder.finish()));
    surface_texture.present();
}
