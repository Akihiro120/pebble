use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub trait GPUSurfaceHandle:
    HasWindowHandle + HasDisplayHandle + Send + Sync + Clone + 'static
{
}

impl<T> GPUSurfaceHandle for T where
    T: HasWindowHandle + HasDisplayHandle + Send + Sync + Clone + 'static
{
}

pub struct WindowConfig {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

pub trait WindowProvider: 'static {
    type Handle;

    fn create(config: &WindowConfig) -> Self;
    fn size(handle: &Self::Handle) -> (u32, u32);
    fn handle(&self) -> &Self::Handle;
}

pub trait PresentableWindow: WindowProvider
where
    Self::Handle: GPUSurfaceHandle,
{
}

pub trait WindowRunner: WindowProvider {
    fn run(self, on_frame: impl FnMut() + 'static);
}

pub struct WindowResource<W: WindowProvider> {
    pub handle: W::Handle,
}
