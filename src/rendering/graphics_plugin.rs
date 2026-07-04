use crate::{
    Backend, Commands, GPUSurfaceHandle, Plugin, RenderTarget, Res, ResMut, SystemStage,
    WindowResource,
};

pub struct GraphicsPlugin<B, W> {
    _marker: std::marker::PhantomData<(B, W)>,
}

impl<B: Backend, W: RenderTarget> GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B: Backend, W: RenderTarget> Plugin for GraphicsPlugin<B, W>
where
    W::Handle: GPUSurfaceHandle,
{
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_system(SystemStage::Startup, setup_gpu::<B, W>)
            .add_resource(LastWindowSize(0, 0))
            .add_system(SystemStage::PreRender, handle_resize::<B, W>);
    }
}

fn setup_gpu<B: Backend, W: RenderTarget>(mut commands: Commands, window: Res<WindowResource<W>>)
where
    W::Handle: GPUSurfaceHandle,
{
    let (w, h) = W::size(&window.handle);

    tracing::info!("Initializing Graphics Backend");
    let backend = B::init(window.handle.clone(), w, h);
    commands.insert_resource(backend);
}

struct LastWindowSize(u32, u32);

fn handle_resize<B: Backend, W: RenderTarget>(
    mut backend: ResMut<B>,
    mut last_size: ResMut<LastWindowSize>,
    window: Res<WindowResource<W>>,
) where
    W::Handle: GPUSurfaceHandle,
{
    let (w, h) = W::size(&window.handle);
    if (w, h) != (last_size.0, last_size.1) && w > 0 && h > 0 {
        backend.resize(w, h);
        *last_size = LastWindowSize(w, h);

        tracing::info!("Window Resized");
    }
}
