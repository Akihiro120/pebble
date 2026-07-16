use crate::{
    prelude::{
        Backend, Commands, GPUSurfaceHandle, Plugin, PresentableWindow, Res, ResMut, SystemStage,
        WindowResource,
    },
    rendering::{sync::init_channel, async_init::PendingBackend},
};

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

fn setup_gpu_async<B: Backend, W>(mut commands: Commands, window: Res<WindowResource<W>>)
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
}

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

fn handle_resize_async<B: Backend, W: PresentableWindow>(
    backend: Option<ResMut<B>>,
    window: Res<WindowResource<W>>,
) where
    W::Handle: GPUSurfaceHandle,
{
    let Some(mut backend) = backend else { return };
    let (w, h) = W::size(&window.handle);
    if w > 0 && h > 0 {
        backend.resize(w, h);
    }
}
