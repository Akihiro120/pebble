use crate::{
    prelude::GPUSurfaceHandle, rendering::errors::AcquireError, rendering::sync::InitSender,
};

/// Describes a render pass: a set of color attachments and an optional depth
/// attachment to render into.
pub struct Pass<'a, F: FrameOperations + ?Sized> {
    /// Color attachments for this pass.
    pub colors: &'a [ColorTarget<'a, F>],
    /// Optional depth attachment for this pass.
    pub depth: Option<DepthTarget<'a, F>>,
}

/// Operations that can be performed on a single acquired frame.
///
/// Implemented by the per-frame type returned from [`Backend::acquire`].
pub trait FrameOperations: Sync + Send + 'static {
    /// The render-pass context (e.g. a command encoder or render pass handle).
    type Context<'a>;
    /// A color attachment (e.g. a texture view).
    type Attachment;
    /// A depth attachment (e.g. a depth-stencil texture view).
    type DepthAttachment;

    /// Begin a render pass and return the rendering context.
    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_>;
}

/// Specifies a color attachment for a render pass.
pub enum ColorTarget<'a, F: FrameOperations + ?Sized> {
    /// Use the backend's default surface attachment.
    Default {
        /// `Some(color)` to clear to that color, `None` to load existing contents.
        clear: Option<[f32; 4]>,
    },
    /// Use a custom attachment (e.g. an off-screen texture).
    Custom {
        attachment: &'a F::Attachment,
        /// `Some(color)` to clear to that color, `None` to load existing contents.
        clear: Option<[f32; 4]>,
    },
}

impl<'a, F: FrameOperations> ColorTarget<'a, F> {
    /// Default attachment, cleared to `clear`.
    pub fn default(clear: [f32; 4]) -> Self {
        Self::Default { clear: Some(clear) }
    }
    /// Default attachment, loading existing contents (no clear).
    pub fn default_load() -> Self {
        Self::Default { clear: None }
    }
    /// Custom attachment, cleared to `clear`.
    pub fn custom(attachment: &'a F::Attachment, clear: [f32; 4]) -> Self {
        Self::Custom {
            attachment,
            clear: Some(clear),
        }
    }
    /// Custom attachment, loading existing contents (no clear).
    pub fn custom_load(attachment: &'a F::Attachment) -> Self {
        Self::Custom {
            attachment,
            clear: None,
        }
    }
}

/// Specifies the depth attachment for a render pass.
pub struct DepthTarget<'a, F: FrameOperations + ?Sized> {
    pub attachment: &'a F::DepthAttachment,
    /// `Some(depth)` to clear to that value, `None` to load existing contents.
    pub clear: Option<f32>,
}

impl<'a, F: FrameOperations> DepthTarget<'a, F> {
    /// Depth attachment cleared to `clear`.
    pub fn new(attachment: &'a F::DepthAttachment, clear: f32) -> Self {
        Self {
            attachment,
            clear: Some(clear),
        }
    }
    /// Depth attachment, loading existing contents (no clear).
    pub fn load(attachment: &'a F::DepthAttachment) -> Self {
        Self {
            attachment,
            clear: None,
        }
    }
}

/// A unified graphics backend for both native and web targets.
///
/// Implement this trait to integrate a concrete graphics API (e.g. wgpu).
/// Initialisation is always done via the [`InitSender`] channel so that
/// implementations can choose to do it synchronously or on a background thread.
pub trait Backend: Sized + Sync + Send + 'static {
    /// The per-frame type that exposes rendering operations.
    type Frame: FrameOperations;

    /// Begin initialisation. Send the finished backend through `sender` when ready.
    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>);

    /// Called when the window is resized. Override to recreate the swapchain.
    fn resize(&mut self, width: u32, height: u32) {
        width;
        height;
    }

    /// Acquire the next frame for rendering.
    ///
    /// Returns [`AcquireError::Transient`] for recoverable failures (e.g.
    /// swapchain out of date) and [`AcquireError::Fatal`] for unrecoverable ones.
    fn acquire(&mut self) -> Result<Self::Frame, AcquireError>;

    /// Present the completed frame to the display.
    fn present(&mut self, frame: Self::Frame);
}

/// Implement for types that can issue draw commands into a render context.
pub trait Drawable<B: Backend> {
    fn draw(&self, pass: &mut <B::Frame as FrameOperations>::Context<'_>);
}

/// Implement for types that can bind themselves (e.g. pipelines, bind groups)
/// into a render context.
pub trait Bindable<B: Backend> {
    fn bind(&self, pass: &mut <B::Frame as FrameOperations>::Context<'_>);
}

/// Resource holding the frame acquired at the start of each render tick.
///
/// Populated by [`RenderPlugin`](crate::rendering::render_plugin::RenderPlugin)
/// during [`PreRender`](crate::app::SystemStage::PreRender) and consumed during
/// [`PostRender`](crate::app::SystemStage::PostRender). Check
/// [`is_active`](CurrentFrame::is_active) before attempting to render.
pub struct CurrentFrame<B: Backend> {
    pub(crate) frame: Option<B::Frame>,
}

impl<B: Backend> CurrentFrame<B> {
    /// Returns `true` if a frame was successfully acquired this tick.
    pub fn is_active(&self) -> bool {
        self.frame.is_some()
    }

    /// Begin a simple full-screen color pass, clearing to `clear`.
    ///
    /// Returns `None` if no frame is active.
    pub fn render_context(
        &mut self,
        clear: [f32; 4],
    ) -> Option<<B::Frame as FrameOperations>::Context<'_>> {
        self.begin_pass(Pass {
            colors: &[ColorTarget::Default { clear: Some(clear) }],
            depth: None,
        })
    }

    /// Begin an arbitrary render pass described by `pass`.
    ///
    /// Returns `None` if no frame is active.
    pub fn begin_pass(
        &mut self,
        pass: Pass<B::Frame>,
    ) -> Option<<B::Frame as FrameOperations>::Context<'_>> {
        self.frame.as_mut().map(|f| f.begin(pass))
    }

    /// Returns a mutable reference to frame
    ///
    /// Returns `None` if no frame is active
    pub fn frame_mut(&mut self) -> Option<&mut B::Frame> {
        self.frame.as_mut()
    }
}
