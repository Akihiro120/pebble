use crate::{
    prelude::{
        Backend, Commands, GPUSurfaceHandle, Plugin, PresentableWindow, Res, ResMut, SystemStage,
        WindowResource,
    },
    rendering::{async_init::PendingBackend, sync::init_channel},
};

/// Plugin that initialises the GPU backend asynchronously and handles window
/// resize events.
///
/// It calls [`Backend::init`] with the window handle and a one-shot sender
/// once (see [`setup_gpu_async`]), then polls the receiver every
/// [`PreRender`](SystemStage::PreRender) tick until the backend arrives. Once
/// available it also forwards window size changes to [`Backend::resize`].
pub struct GraphicsPlugin<B, W> {
    _marker: std::marker::PhantomData<(B, W)>,
}

impl<B: Backend, W: PresentableWindow> GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B: Backend, W: PresentableWindow> Plugin for GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_system(SystemStage::Startup, setup_gpu_async::<B, W>)
            .add_system(SystemStage::PreRender, poll_backend_ready::<B>)
            .add_system(SystemStage::PreRender, handle_resize_async::<B, W>);
    }
}

struct LastWindowSize(u32, u32);

/// Startup system: kicks off backend initialisation and stores the pending
/// receiver. `WindowResource<W>` already exists by the time this first runs
/// (inserted synchronously by `WindowPlugin::build`), so this always succeeds
/// on its first invocation and is never invoked again.
fn setup_gpu_async<B: Backend, W>(mut commands: Commands, window: Res<WindowResource<W>>) -> Option<()>
where
    W: PresentableWindow,
    W::Handle: GPUSurfaceHandle,
{
    let (w, h) = W::size(&window.handle);
    let (sender, receiver) = init_channel::<B>();
    B::init(window.handle.clone(), w, h, sender);
    commands.insert_resource(PendingBackend::<B> {
        receiver: std::sync::Mutex::new(receiver),
    });
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
