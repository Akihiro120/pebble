use crate::{errors::AcquireError, prelude::GPUSurfaceHandle, rendering::sync::InitSender};

pub struct Pass<'a, F: FrameOperations + ?Sized> {
    pub colors: &'a [ColorTarget<'a, F>],
    pub depth: Option<DepthTarget<'a, F>>,
}

pub trait FrameOperations: Sync + Send + 'static {
    type Context<'a>;
    type Attachment;
    type DepthAttachment;

    fn default_target(&self) -> &Self::Attachment;
    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_>;
}

pub enum ColorTarget<'a, F: FrameOperations + ?Sized> {
    Default {
        clear: Option<[f32; 4]>,
    },
    Custom {
        attachment: &'a F::Attachment,
        clear: Option<[f32; 4]>,
    },
}

impl<'a, F: FrameOperations> ColorTarget<'a, F> {
    pub fn default(clear: [f32; 4]) -> Self {
        Self::Default { clear: Some(clear) }
    }
    pub fn default_load() -> Self {
        Self::Default { clear: None }
    }
    pub fn custom(attachment: &'a F::Attachment, clear: [f32; 4]) -> Self {
        Self::Custom {
            attachment,
            clear: Some(clear),
        }
    }
    pub fn custom_load(attachment: &'a F::Attachment) -> Self {
        Self::Custom {
            attachment,
            clear: None,
        }
    }
}

pub struct DepthTarget<'a, F: FrameOperations + ?Sized> {
    pub attachment: &'a F::DepthAttachment,
    pub clear: Option<f32>,
}

impl<'a, F: FrameOperations> DepthTarget<'a, F> {
    pub fn new(attachment: &'a F::DepthAttachment, clear: f32) -> Self {
        Self {
            attachment,
            clear: Some(clear),
        }
    }
    pub fn load(attachment: &'a F::DepthAttachment) -> Self {
        Self {
            attachment,
            clear: None,
        }
    }
}

pub trait Backend: Sized + Sync + Send + 'static {
    type Frame: FrameOperations;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>);
    fn resize(&mut self, width: u32, height: u32) {
        width;
        height;
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError>;
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

    pub fn render_context(
        &mut self,
        clear: [f32; 4],
    ) -> Option<<B::Frame as FrameOperations>::Context<'_>> {
        self.begin_pass(Pass {
            colors: &[ColorTarget::Default { clear: Some(clear) }],
            depth: None,
        })
    }

    pub fn begin_pass(
        &mut self,
        pass: Pass<B::Frame>,
    ) -> Option<<B::Frame as FrameOperations>::Context<'_>> {
        self.frame.as_mut().map(|f| f.begin(pass))
    }
}
