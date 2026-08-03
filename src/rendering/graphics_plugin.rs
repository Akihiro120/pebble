use crate::{
    ecs::system::OnceExt,
    prelude::{
        Backend, Commands, GPUSurfaceHandle, Plugin, PresentableWindow, Res, ResMut, SystemStage,
        WindowResource,
    },
    rendering::{async_init::PendingBackend, sync::init_channel},
};

/// Plugin that initialises the GPU backend asynchronously and handles window
/// resize events.
///
/// It calls [`Backend::init`] with the window handle, the backend's
/// [`InitConfig`](Backend::InitConfig) (see [`with_config`](Self::with_config))
/// and a one-shot sender once (see [`setup_gpu_async`]), then polls the
/// receiver every [`PreRender`](SystemStage::PreRender) tick until the
/// backend arrives. Once available it also forwards window size changes to
/// [`Backend::resize`].
pub struct GraphicsPlugin<B: Backend, W> {
    config: B::InitConfig,
    _marker: std::marker::PhantomData<(B, W)>,
}

impl<B: Backend, W: PresentableWindow> GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    pub fn new() -> Self {
        Self {
            config: B::InitConfig::default(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Supply a backend-specific initialisation config. Pebble does not
    /// inspect it — its meaning belongs entirely to the backend.
    pub fn with_config(mut self, config: B::InitConfig) -> Self {
        self.config = config;
        self
    }
}

impl<B: Backend, W: PresentableWindow> Plugin for GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    fn build(&self, app: &mut crate::prelude::App) {
        // B arrives asynchronously (see poll_backend_ready) — mark it so a
        // system elsewhere with a hard `Res<B>` requirement waits quietly
        // instead of App treating it as a missing/misconfigured resource.
        app.provides::<B>();
        // Systems are plain functions, so the config reaches setup_gpu_async
        // as a resource; it's removed again once consumed.
        app.add_resource(BackendInitConfig::<B>(self.config.clone()));
        app.add_system(SystemStage::PreUpdate, setup_gpu_async::<B, W>.once())
            .add_system(SystemStage::PreRender, poll_backend_ready::<B>)
            .add_system(SystemStage::PreRender, handle_resize_async::<B, W>);
    }
}

struct LastWindowSize(u32, u32);

/// Carrier resource moving the app-supplied [`Backend::InitConfig`] from
/// plugin registration to [`setup_gpu_async`]. Removed after use.
struct BackendInitConfig<B: Backend>(B::InitConfig);

/// Kicks off backend initialisation and stores the pending receiver.
/// `.once()`-registered — `WindowResource<W>` already exists by the time this
/// first runs (inserted synchronously by `WindowPlugin::build`), so this
/// always succeeds on its first invocation and is never invoked again.
fn setup_gpu_async<B: Backend, W>(
    mut commands: Commands,
    window: Res<WindowResource<W>>,
    config: Res<BackendInitConfig<B>>,
) -> Option<()>
where
    W: PresentableWindow,
    W::Handle: GPUSurfaceHandle,
{
    let (w, h) = W::size(&window.handle);
    let (sender, receiver) = init_channel::<B>();
    B::init(window.handle.clone(), w, h, sender, config.0.clone());
    commands.insert_resource(PendingBackend::<B> {
        receiver: std::sync::Mutex::new(receiver),
    });
    commands.remove_resource::<BackendInitConfig<B>>();
    Some(())
}

/// PreRender system: poll the one-shot channel; promote the backend to a
/// resource and remove the pending marker once it arrives.
fn poll_backend_ready<B: Backend>(mut commands: Commands, pending: Option<Res<PendingBackend<B>>>) {
    if let Some(p) = pending {
        let mut guard = match p.receiver.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Ok(backend) = guard.try_recv() {
            commands.insert_resource(backend);
            commands.remove_resource::<PendingBackend<B>>();
        }
    }
}

/// PreRender system: forward the current window size to the backend so it can
/// recreate the swapchain when the window is resized.
///
/// `Backend::resize` reconfigures the surface, which is expensive (it drains
/// the GPU queue and recreates the swapchain), so this only calls it when the
/// size has actually changed rather than unconditionally every frame.
fn handle_resize_async<B: Backend, W: PresentableWindow>(
    mut commands: Commands,
    backend: Option<ResMut<B>>,
    window: Res<WindowResource<W>>,
    last_size: Option<Res<LastWindowSize>>,
) where
    W::Handle: GPUSurfaceHandle,
{
    let Some(mut backend) = backend else { return };
    let (w, h) = W::size(&window.handle);
    if w == 0 || h == 0 {
        return;
    }

    if let Some(last_size) = &last_size {
        if last_size.0 == w && last_size.1 == h {
            return;
        }
    }

    backend.resize(w, h);
    commands.insert_resource(LastWindowSize(w, h));
}
