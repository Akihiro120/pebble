use crate::{
    prelude::{Backend, Commands, CurrentFrame, Plugin, Res, ResMut, SystemStage},
    rendering::errors::AcquireError,
};

/// Inserted the first time [`Backend::acquire`] returns
/// [`AcquireError::Fatal`] — the render loop has permanently stopped
/// producing frames (a lost device, a destroyed surface, anything the
/// backend itself judged unrecoverable). Once this exists, `begin_frame`
/// stops calling `acquire` at all, so the same fatal condition isn't
/// re-triggered and re-logged every tick forever.
///
/// The framework deliberately does *not* panic or exit on your behalf here
/// — a hard crash isn't always the right response, and only your
/// application knows whether the right move is an error screen, a full
/// backend re-init, or something else. Check for it explicitly wherever
/// that decision belongs:
///
/// ```ignore
/// fn on_render_death(failure: Res<RenderFailure>) -> Option<()> {
///     eprintln!("rendering has permanently stopped: {}", failure.message);
///     Some(()) // .once() — react exactly once, not every tick thereafter
/// }
///
/// app.add_system(SystemStage::PostRender, on_render_death.once());
/// ```
///
/// `RenderPlugin` declares this as [provided](crate::app::App::provides), so
/// a system with a hard `Res<RenderFailure>` requirement (as in the example
/// above) waits quietly for it rather than panicking at startup over a
/// resource that, in the common case where rendering never fails, is
/// correctly never going to appear.
pub struct RenderFailure {
    pub message: String,
}

/// Plugin that manages the per-frame acquire / present cycle.
///
/// Adds a [`CurrentFrame<B>`] resource and two systems:
/// - [`PreRender`](SystemStage::PreRender): acquires a frame from the backend.
/// - [`PostRender`](SystemStage::PostRender): presents the completed frame.
///
/// Rendering systems should check [`CurrentFrame::is_active`] before issuing
/// draw calls, as the frame may be absent when the backend is not yet ready or
/// a transient acquire error occurs. See [`RenderFailure`] for the
/// permanent-failure case specifically.
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
            .provides::<RenderFailure>()
            .add_system(SystemStage::PreRender, begin_frame::<B>)
            .add_system(SystemStage::PostRender, end_frame::<B>);
    }
}

/// PreRender system: acquire the next frame. Clears the current frame on a
/// transient error; on a fatal one, logs it once and inserts
/// [`RenderFailure`] instead of retrying forever.
fn begin_frame<B: Backend>(
    backend: Option<ResMut<B>>,
    mut frame: ResMut<CurrentFrame<B>>,
    already_failed: Option<Res<RenderFailure>>,
    mut commands: Commands,
) {
    // Already permanently failed — nothing acquire() could tell us now
    // changes that, so don't keep calling into a backend that may itself be
    // in a broken state.
    if already_failed.is_some() {
        return;
    }

    let Some(mut backend) = backend else { return };

    match backend.acquire() {
        Ok(f) => frame.frame = Some(f),
        Err(AcquireError::Transient) => frame.frame = None,
        Err(AcquireError::Fatal(msg)) => {
            tracing::error!("Fatal frame acquisition error — rendering has permanently stopped: {msg}");
            frame.frame = None;
            commands.insert_resource(RenderFailure { message: msg });
        }
    }
}

/// PostRender system: present the completed frame to the display.
///
/// `pub(crate)` (not private) so other in-crate plugins that draw directly
/// onto the frame after the app's own render systems but before it's
/// presented — the `profiler` feature's overlay, currently the only such
/// case — can order themselves against it via
/// [`SystemOrderingExt::before`](crate::ecs::system::SystemOrderingExt::before)
/// instead of depending on registration order.
pub(crate) fn end_frame<B: Backend>(backend: Option<ResMut<B>>, mut current: ResMut<CurrentFrame<B>>) {
    let Some(mut backend) = backend else { return };
    if let Some(frame) = current.frame.take() {
        backend.present(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::rendering::backend::{FrameOperations, Pass};
    use crate::rendering::sync::InitSender;
    use crate::rendering::window::GPUSurfaceHandle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct FakeFrame;
    impl FrameOperations for FakeFrame {
        type Context<'a> = ();
        type Attachment = ();
        type DepthAttachment = ();
        fn begin(&mut self, _pass: Pass<'_, Self>) -> Self::Context<'_> {}
    }

    struct FakeBackend {
        acquire_calls: Arc<AtomicU32>,
    }

    impl Backend for FakeBackend {
        type Frame = FakeFrame;

        fn init(_handle: impl GPUSurfaceHandle, _width: u32, _height: u32, _sender: InitSender<Self>) {
            unreachable!("not exercised by this test — the backend is inserted directly")
        }

        fn acquire(&mut self) -> Result<Self::Frame, AcquireError> {
            self.acquire_calls.fetch_add(1, Ordering::SeqCst);
            Err(AcquireError::Fatal("simulated device loss".to_string()))
        }

        fn present(&mut self, _frame: Self::Frame) {}
    }

    #[test]
    fn a_fatal_acquire_error_reports_failure_and_stops_retrying() {
        let acquire_calls = Arc::new(AtomicU32::new(0));
        let backend = FakeBackend { acquire_calls: acquire_calls.clone() };

        let mut app = App::new();
        app.add_resource(backend);
        app.add_plugin(RenderPlugin::<FakeBackend>::new());
        app.build();

        app.update();
        assert_eq!(acquire_calls.load(Ordering::SeqCst), 1);
        assert_eq!(app.get_resource::<RenderFailure>().message, "simulated device loss");

        // Two more ticks: acquire() must not be called again now that
        // RenderFailure exists — this is the "stop retrying forever"
        // half of the fix, not just "report it once".
        app.update();
        app.update();
        assert_eq!(
            acquire_calls.load(Ordering::SeqCst),
            1,
            "begin_frame kept calling acquire() after a Fatal error instead of stopping"
        );
    }
}
