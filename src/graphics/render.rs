use std::sync::Arc;

use crate::{
    app::{AppExit, BackendReady},
    ecs::{
        commands::Commands,
        plugin::Plugin,
        promise::{Promise, PromiseState},
        resources::{Read, Write},
    },
    graphics::{
        render::frame::{CurrentFrame, Frame},
        types::{TextureFormat, flags::DeviceFeatures, limits::DeviceLimits},
        window::Window,
    },
};

pub mod compute_pass;
pub mod frame;
pub(crate) mod gpu_context;
pub mod render_pass;
pub mod targets;
pub mod timestamps;

/// The GPU device/queue/surface — inserted as a resource once acquisition
/// finishes (see [`Read<Backend>`](crate::ecs::resources::Read), safe to
/// use unwrapped anywhere from [`SystemStage::Ready`](crate::ecs::system::SystemStage::Ready)
/// onward). No raw `wgpu` type is exposed on it besides through the
/// dedicated escape hatches on the wrapper types built on top of it.
pub struct Backend {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_configuration: wgpu::SurfaceConfiguration,
    features: DeviceFeatures,
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

    pub fn features(&self) -> DeviceFeatures {
        self.features.into()
    }

    pub fn limits(&self) -> DeviceLimits {
        self.device.limits().into()
    }

    /// Records and submits a compute pass immediately, in its own command
    /// encoder — not deferred to any render stage, since compute work
    /// doesn't need a frame to exist. Read a result back with
    /// [`Buffer::read`](crate::graphics::pipeline::buffers::Buffer::read)
    /// once the pass has run.
    pub fn dispatch_compute(&self, record: impl FnOnce(&mut compute_pass::ComputePass)) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let raw_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            let mut pass = compute_pass::ComputePass::new(raw_pass);
            record(&mut pass);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}

/// Why the GPU could not be acquired. Handed to
/// [`GraphicsPlugin::on_backend_error`](crate::graphics::GraphicsPlugin::on_backend_error),
/// so a host can tell a human which of these it was.
#[derive(Debug, Clone)]
pub enum BackendError {
    /// The window would not give wgpu a surface. On the web,
    /// `canvas.getContext("webgpu")` returned null — the canvas already holds
    /// another kind of context.
    Surface(String),
    /// No adapter. On the web this is either no `navigator.gpu` at all, or —
    /// the case a host cannot check for in advance — `requestAdapter()`
    /// resolving to null: the browser has WebGPU but will not use this
    /// machine's GPU for it (a blocklisted driver, hardware acceleration off,
    /// a remote desktop or virtual machine).
    NoAdapter(String),
    /// The adapter would not create a device.
    Device(String),
    /// The surface offers no non-sRGB format, which this renderer needs.
    SurfaceFormat(String),
    /// Both the preferred adapter/device and the software fallback failed.
    Fallback { primary: String, fallback: String },
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(e) => write!(f, "could not create a drawing surface: {e}"),
            Self::NoAdapter(e) => write!(f, "no usable GPU adapter: {e}"),
            Self::Device(e) => write!(f, "the GPU adapter would not grant a device: {e}"),
            Self::SurfaceFormat(formats) => {
                write!(f, "the surface offers no non-sRGB format (got {formats})")
            }
            Self::Fallback { primary, fallback } => write!(
                f,
                "normal GPU initialization failed ({primary}); fallback initialization also failed ({fallback})"
            ),
        }
    }
}

impl std::error::Error for BackendError {}

pub(crate) type BackendErrorReport = Box<dyn FnOnce(&BackendError) + Send + Sync>;

/// The host's [`BackendError`] callback, taken the one time it is called.
pub(crate) struct BackendErrorHook(Option<BackendErrorReport>);

/// Present once acquisition has failed for good, so `obtain_gpu` does not
/// start over.
pub(crate) struct BackendFailed;

pub(crate) struct BackendPlugin {
    features: DeviceFeatures,
    on_error: Option<BackendErrorReport>,
}

pub(crate) struct BackendConfig {
    features: DeviceFeatures,
}

impl BackendPlugin {
    pub fn with_features(features: DeviceFeatures) -> Self {
        Self {
            features,
            on_error: None,
        }
    }

    pub fn on_error(mut self, report: Option<BackendErrorReport>) -> Self {
        self.on_error = report;
        self
    }
}

impl Plugin for BackendPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.insert_resource(BackendConfig {
            features: self.features,
        })
        .insert_resource(BackendErrorHook(self.on_error))
        .insert_resource(CurrentFrame::default())
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
    promise: Promise<Result<Backend, BackendError>>,
}

async fn with_adapter_fallback<T, F, Fut>(mut acquire: F) -> Result<T, BackendError>
where
    F: FnMut(bool) -> Fut,
    Fut: std::future::Future<Output = Result<T, BackendError>>,
{
    match acquire(false).await {
        Ok(value) => Ok(value),
        Err(primary) => {
            tracing::warn!(
                "normal GPU initialization failed: {primary}; trying a fallback adapter"
            );
            acquire(true)
                .await
                .map_err(|fallback| BackendError::Fallback {
                    primary: primary.to_string(),
                    fallback: fallback.to_string(),
                })
        }
    }
}

async fn acquire_device(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
    features: DeviceFeatures,
    fallback: bool,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue, wgpu::Features), BackendError> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if fallback {
                wgpu::PowerPreference::None
            } else {
                wgpu::PowerPreference::HighPerformance
            },
            compatible_surface: Some(surface),
            force_fallback_adapter: fallback,
            apply_limit_buckets: true,
        })
        .await
        .map_err(|e| BackendError::NoAdapter(e.to_string()))?;
    let required_features = Into::<wgpu::Features>::into(features) & adapter.features();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            required_features,
            ..Default::default()
        })
        .await
        .map_err(|e| BackendError::Device(e.to_string()))?;
    tracing::info!(
        fallback_requested = fallback,
        adapter = ?adapter.get_info(),
        features = ?required_features,
        limits = ?device.limits(),
        "GPU device acquired"
    );
    Ok((adapter, device, queue, required_features))
}

/// Acquire the GPU, or say why it could not be.
///
/// Fallible rather than panicking because the most likely reason to fail is a
/// browser without a usable WebGPU, and a wasm trap there is the worst of both
/// worlds: the host application gets an unhandleable abort instead of an error
/// it could have shown a human. `poll_gpu` hands the error to the host.
async fn init_gpu(
    window: Arc<winit::window::Window>,
    config: BackendConfig,
) -> Result<Backend, BackendError> {
    let instance = wgpu::Instance::default();

    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| BackendError::Surface(e.to_string()))?;

    // Best-effort: an optional feature the adapter doesn't support is
    // dropped rather than failing device creation. `Backend::features`
    // reports what was actually granted.
    let (adapter, device, queue, required_features) = with_adapter_fallback(|fallback| {
        acquire_device(&instance, &surface, config.features, fallback)
    })
    .await?;

    // On the web the window may not have a size yet (see `web_canvas_size`)
    // — a zero configure is invalid, so clamp; `begin_frame` reconfigures to
    // the real size as soon as it has one.
    let window_size = window.inner_size();

    let caps = surface.get_capabilities(&adapter);
    // A surface with no non-sRGB format is not one this renderer can drive, and
    // that is worth saying rather than indexing off the end of the list.
    let format = caps
        .formats
        .iter()
        .find(|f| !f.is_srgb())
        .ok_or_else(|| BackendError::SurfaceFormat(format!("{:?}", caps.formats)))?
        .clone();
    let surface_configuration = wgpu::SurfaceConfiguration {
        alpha_mode: caps.alpha_modes[0],
        present_mode: caps.present_modes[0],
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: window_size.width.max(1),
        height: window_size.height.max(1),
        color_space: wgpu::SurfaceColorSpace::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_configuration);

    Ok(Backend {
        device,
        queue,
        surface,
        surface_configuration,
        features: required_features.into(),
    })
}

pub(crate) fn obtain_gpu(
    mut commands: Commands,
    window: Read<Window>,
    receiver: Option<Read<GPUReceiver>>,
    failed: Option<Read<BackendFailed>>,
    config: Read<BackendConfig>,
) {
    // already waiting on a previous request, or it failed and asking again
    // would only fail again
    if receiver.is_some() || failed.is_some() {
        return;
    }

    let window = window.raw();
    let (fulfiller, promise) = Promise::new();
    let features = config.features;

    let fut = async move {
        fulfiller.fulfill(init_gpu(window, BackendConfig { features }).await);
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
    hook: Option<Write<BackendErrorHook>>,
) {
    let Some(receiver) = receiver else {
        return;
    };

    if !ready.0 {
        // continue polling for the gpu
        match receiver.promise.poll() {
            PromiseState::Ready(Ok(backend)) => {
                tracing::info!("GPU backend ready, adding as resource");
                commands.insert_resource(backend);
                ready.0 = true;
            }
            PromiseState::Ready(Err(error)) => {
                tracing::error!("GPU backend unavailable: {error}");
                if let Some(report) = hook.and_then(|mut hook| hook.0.take()) {
                    report(&error);
                }
                give_up_on_gpu(&mut commands);
            }
            PromiseState::Pending => {}
            // `init_gpu` reports either way, so only a future dropped before it
            // finished gets here.
            PromiseState::Disconnected => {
                tracing::error!("GPU backend init ended without reporting a result");
                give_up_on_gpu(&mut commands);
            }
        }
    }
}

/// With no backend, no system of the app's own will ever run, so there is
/// nothing left to wait for. Ending the app beats the alternative: a runner
/// asking for frames forever to poll for a backend that is not coming, and a
/// host left unable to shut down an app whose shutdown systems never run.
fn give_up_on_gpu(commands: &mut Commands) {
    commands.remove_resource::<GPUReceiver>();
    commands.insert_resource(BackendFailed);
    commands.insert_resource(AppExit(true));
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

/// The canvas's laid-out size in physical pixels, `None` until it has one.
#[cfg(target_arch = "wasm32")]
fn web_canvas_size(window: &Window) -> Option<(u32, u32)> {
    use winit::platform::web::WindowExtWebSys;
    let canvas = window.raw().canvas()?;
    let rect = canvas.get_bounding_client_rect();
    let scale = web_sys::window()?.device_pixel_ratio();
    let (width, height) = (
        (rect.width() * scale) as u32,
        (rect.height() * scale) as u32,
    );
    (width > 0 && height > 0).then_some((width, height))
}

pub(crate) fn begin_frame(
    mut backend: Write<Backend>,
    mut current_frame: Write<CurrentFrame>,
    window: Read<Window>,
) {
    // On the web winit's tracked size arrives via ResizeObserver, which
    // Safari delivers late or not at all — measure the canvas directly so
    // the surface is never configured to a stale or zero size.
    #[cfg(target_arch = "wasm32")]
    let (width, height) = web_canvas_size(&window).unwrap_or_else(|| window.inner_size());
    #[cfg(not(target_arch = "wasm32"))]
    let (width, height) = window.inner_size();
    if width > 0
        && height > 0
        && (width != backend.surface_configuration.width
            || height != backend.surface_configuration.height)
    {
        backend.surface_configuration.width = width;
        backend.surface_configuration.height = height;
        backend
            .surface
            .configure(&backend.device, &backend.surface_configuration);
    }

    let surface_texture = match backend.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture) => texture,
        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            backend
                .surface
                .configure(&backend.device, &backend.surface_configuration);
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            tracing::error!("surface validation error acquiring the next frame");
            return;
        }
    };

    let view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    current_frame.set(Frame::new(backend.device.clone(), view, surface_texture));
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

    let (buffers, surface_texture) = frame.finish();
    backend.queue.submit(buffers);
    backend.queue.present(surface_texture);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{app::App, ecs::system::SystemStage};

    #[test]
    fn successful_primary_does_not_request_fallback() {
        let mut attempts = Vec::new();
        let result = pollster::block_on(with_adapter_fallback(|fallback| {
            attempts.push(fallback);
            std::future::ready(Ok(42))
        }));
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, [false]);
    }

    #[test]
    fn missing_adapter_and_denied_device_both_try_fallback() {
        for device_denied in [false, true] {
            let mut attempts = Vec::new();
            let result = pollster::block_on(with_adapter_fallback(|fallback| {
                attempts.push(fallback);
                std::future::ready(if fallback {
                    Ok(42)
                } else if device_denied {
                    Err(BackendError::Device("primary device rejected".into()))
                } else {
                    Err(BackendError::NoAdapter(
                        "primary adapter unavailable".into(),
                    ))
                })
            }));
            assert_eq!(result.unwrap(), 42);
            assert_eq!(attempts, [false, true]);
        }
    }

    #[test]
    fn both_failures_preserve_the_causes_without_retrying_forever() {
        let mut attempts = Vec::new();
        let result: Result<(), _> = pollster::block_on(with_adapter_fallback(|fallback| {
            attempts.push(fallback);
            std::future::ready(Err(if fallback {
                BackendError::Device("fallback device rejected".into())
            } else {
                BackendError::NoAdapter("primary adapter unavailable".into())
            }))
        }));
        let message = result.unwrap_err().to_string();
        assert!(message.contains("primary adapter unavailable"));
        assert!(message.contains("fallback device rejected"));
        assert_eq!(attempts, [false, true]);
    }

    #[derive(Default)]
    struct Observed {
        reports: Vec<String>,
        waiting: bool,
        failed: bool,
    }

    /// A failed acquisition reports once, stops polling and asking, and ends
    /// the app — rather than logging on every tick of a loop that asks for
    /// frames forever.
    #[test]
    fn a_failed_acquisition_reports_once_and_exits() {
        let observed = Arc::new(Mutex::new(Observed::default()));
        let (fulfiller, promise) = Promise::new();
        fulfiller.fulfill(Err(BackendError::NoAdapter(
            "requestAdapter() resolved to null".into(),
        )));

        let reports = observed.clone();
        let watch = observed.clone();
        let mut app = App::new()
            .insert_resource(GPUReceiver { promise })
            .insert_resource(BackendErrorHook(Some(Box::new(
                move |error: &BackendError| {
                    reports.lock().unwrap().reports.push(error.to_string());
                },
            ))))
            .add_gpu_system(SystemStage::Update, poll_gpu)
            .add_gpu_system(
                SystemStage::PostUpdate,
                move |receiver: Option<Read<GPUReceiver>>, failed: Option<Read<BackendFailed>>| {
                    let mut seen = watch.lock().unwrap();
                    seen.waiting = receiver.is_some();
                    seen.failed = failed.is_some();
                },
            );

        app.update();
        app.update();

        let seen = observed.lock().unwrap();
        assert_eq!(
            seen.reports,
            vec!["no usable GPU adapter: requestAdapter() resolved to null".to_string()]
        );
        assert!(
            !seen.waiting,
            "the receiver is dropped, so nothing polls it again"
        );
        assert!(seen.failed, "obtain_gpu is told not to start over");
        assert!(app.should_exit());
        assert!(!app.backend_ready());
    }
}
