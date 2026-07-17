use crate::{
    rendering::errors::AcquireError,
    prelude::{Backend, CurrentFrame, Plugin, ResMut, SystemStage},
};

/// Plugin that manages the per-frame acquire / present cycle.
///
/// Adds a [`CurrentFrame<B>`] resource and two systems:
/// - [`PreRender`](SystemStage::PreRender): acquires a frame from the backend.
/// - [`PostRender`](SystemStage::PostRender): presents the completed frame.
///
/// Rendering systems should check [`CurrentFrame::is_active`] before issuing
/// draw calls, as the frame may be absent when the backend is not yet ready or
/// a transient acquire error occurs.
pub struct RenderPlugin<B: Backend> {
    _marker: std::marker::PhantomData<B>,
}

impl<B: Backend> RenderPlugin<B> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B: Backend> Plugin for RenderPlugin<B> {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_resource(CurrentFrame::<B> { frame: None })
            .add_system(SystemStage::PreRender, begin_frame::<B>)
            .add_system(SystemStage::PostRender, end_frame::<B>);
    }
}

/// PreRender system: acquire the next frame. Clears the current frame on
/// transient errors and logs fatal ones.
fn begin_frame<B: Backend>(backend: Option<ResMut<B>>, mut frame: ResMut<CurrentFrame<B>>) {
    let Some(mut backend) = backend else { return };

    match backend.acquire() {
        Ok(f) => frame.frame = Some(f),
        Err(AcquireError::Transient) => frame.frame = None,
        Err(AcquireError::Fatal(msg)) => {
            tracing::error!("Fatal frame acquisition error: {msg}");
            frame.frame = None;
        }
    }
}

/// PostRender system: present the completed frame to the display.
fn end_frame<B: Backend>(backend: Option<ResMut<B>>, mut current: ResMut<CurrentFrame<B>>) {
    let Some(mut backend) = backend else { return };
    if let Some(frame) = current.frame.take() {
        backend.present(frame);
    }
}
