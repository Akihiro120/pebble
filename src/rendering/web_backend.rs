use crate::{
    prelude::{
        Backend, Commands, GPUSurfaceHandle, Plugin, PresentableWindow, Res, ResMut, SystemStage,
        WindowResource,
    },
    rendering::sync::{InitReceiver, InitSender, init_channel},
};

pub trait AsyncInit: Backend {
    fn init_async(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>);
}

pub struct PendingBackend<B: AsyncInit> {
    receiver: std::sync::Mutex<InitReceiver<B>>,
}

pub struct AsyncGraphicsPlugin<B, W>(std::marker::PhantomData<(B, W)>);

impl<B, W> AsyncGraphicsPlugin<B, W> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<B: AsyncInit, W> Plugin for AsyncGraphicsPlugin<B, W>
where
    W: PresentableWindow,
    W::Handle: GPUSurfaceHandle,
{
    fn build(&self, app: &mut crate::app::App) {
        app.add_system(SystemStage::Startup, setup_gpu_async::<B, W>)
            .add_system(SystemStage::PreRender, poll_backend_ready::<B>)
            .add_system(SystemStage::PreRender, handle_resize_async::<B, W>);
    }
}

fn setup_gpu_async<B: AsyncInit, W>(mut commands: Commands, window: Res<WindowResource<W>>)
where
    W: PresentableWindow,
    W::Handle: GPUSurfaceHandle,
{
    let (w, h) = W::size(&window.handle);
    let (sender, receiver) = init_channel::<B>();
    B::init_async(window.handle.clone(), w, h, sender);
    commands.insert_resource(PendingBackend::<B> {
        receiver: std::sync::Mutex::new(receiver),
    });
}

fn poll_backend_ready<B: AsyncInit>(
    mut commands: Commands,
    pending: Option<Res<PendingBackend<B>>>,
) {
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

fn handle_resize_async<B: AsyncInit, W: PresentableWindow>(
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
