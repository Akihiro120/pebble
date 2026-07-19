use crate::prelude::{Backend, ColorTarget, FrameOperations, Pass};

pub struct ActiveFrame<'a, B: Backend> {
    pub(crate) frame: &'a mut B::Frame,
}

impl<'a, B: Backend> ActiveFrame<'a, B> {
    /// Begin a simple full-screen color pass, clearing to `clear`.
    pub fn render_context(
        &mut self,
        clear: [f32; 4],
    ) -> <B::Frame as FrameOperations>::Context<'_> {
        self.begin_pass(Pass {
            colors: &[ColorTarget::Default { clear: Some(clear) }],
            depth: None,
        })
    }

    /// Begin an arbitrary render pass described by `pass`.
    pub fn begin_pass(
        &mut self,
        pass: Pass<B::Frame>,
    ) -> <B::Frame as FrameOperations>::Context<'_> {
        // self.frame.as_mut().map(|f| f.begin(pass))
        self.frame.begin(pass)
    }
}

impl<'a, B: Backend> std::ops::Deref for ActiveFrame<'a, B> {
    type Target = B::Frame;
    fn deref(&self) -> &Self::Target {
        self.frame
    }
}

impl<'a, B: Backend> std::ops::DerefMut for ActiveFrame<'a, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.frame
    }
}
