use crate::GPUSurfaceHandle;

pub trait FrameOperations: Sync + Send + 'static {
    type Context;
    fn context(&mut self) -> &mut Self::Context;
}

pub trait Backend: Sized + Sync + Send + 'static {
    type Frame: FrameOperations;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32) -> Self;
    fn resize(&mut self, width: u32, height: u32);

    fn acquire(&mut self) -> Option<Self::Frame>;
    fn present(&mut self, frame: Self::Frame);
}

pub trait Drawable<B: Backend> {
    fn draw(&self, ctx: &mut <B::Frame as FrameOperations>::Context);
}

pub trait Bindable<B: Backend> {
    fn bind(&self, ctx: &mut <B::Frame as FrameOperations>::Context);
}

pub struct CurrentFrame<B: Backend> {
    pub frame: Option<B::Frame>,
}

impl<B: Backend> CurrentFrame<B> {
    pub fn is_active(&self) -> bool {
        self.frame.is_some()
    }

    pub fn context(&mut self) -> Option<&mut <B::Frame as FrameOperations>::Context> {
        self.frame.as_mut().map(|f| f.context())
    }
}
