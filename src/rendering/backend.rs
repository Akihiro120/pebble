use crate::GPUSurfaceHandle;

pub trait FrameOperations: Sync + Send + 'static {
    type Context<'a>;
    fn context(&mut self) -> Self::Context<'_>;
}

pub trait Backend: Sized + Sync + Send + 'static {
    type Frame: FrameOperations;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32) -> Self;
    fn resize(&mut self, width: u32, height: u32);

    fn acquire(&mut self) -> Option<Self::Frame>;
    fn present(&mut self, frame: Self::Frame);
}

pub trait Drawable<B: Backend> {
    fn draw(&self, pass: &mut <B::Frame as FrameOperations>::Context<'_>);
}

pub trait Bindable<B: Backend> {
    fn bind(&self, pass: &mut <B::Frame as FrameOperations>::Context<'_>);
}

pub struct CurrentFrame<B: Backend> {
    pub(crate) frame: Option<B::Frame>,
}

impl<B: Backend> CurrentFrame<B> {
    pub fn is_active(&self) -> bool {
        self.frame.is_some()
    }

    pub fn get_render_context(&mut self) -> Option<<B::Frame as FrameOperations>::Context<'_>> {
        self.frame.as_mut().map(|frame| frame.context())
    }
}
