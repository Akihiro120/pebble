use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// A window handle that can be used to create a GPU surface.
///
/// Automatically implemented for any type that implements
/// [`HasWindowHandle`], [`HasDisplayHandle`], `Send`, `Sync`, and `Clone`.
pub trait GPUSurfaceHandle:
    HasWindowHandle + HasDisplayHandle + Send + Sync + Clone + 'static
{
}

impl<T> GPUSurfaceHandle for T where
    T: HasWindowHandle + HasDisplayHandle + Send + Sync + Clone + 'static
{
}

/// Configuration passed to a [`WindowProvider`] when creating a window.
pub struct WindowConfig {
    pub title: &'static str,
    pub width: u32,
    pub height: u32,
}

/// Abstracts over a platform-specific window implementation.
///
/// Implement this trait to plug in any windowing library (e.g. winit). The
/// associated types expose the window handle used for surface creation and an
/// "exposed" value (e.g. a shareable Arc) that systems can inspect.
pub trait WindowProvider: 'static {
    /// The concrete window handle type used to create a GPU surface.
    type Handle: GPUSurfaceHandle;
    /// An additional value derived from the window that can be cloned and
    /// shared with other parts of the app (e.g. an `Arc<Window>`).
    type Exposed: Clone + Send + Sync + 'static;

    /// Create a window using the provided configuration.
    fn create(config: &WindowConfig) -> Self;
    /// Return the current inner size of `handle` in physical pixels.
    fn size(handle: &Self::Handle) -> (u32, u32);
    /// Return the exposed value for this window.
    fn exposed(&self) -> Self::Exposed;
    /// Return a reference to the raw window handle.
    fn handle(&self) -> &Self::Handle;
}

/// Marker trait for window providers whose handle can be used as a GPU surface.
pub trait PresentableWindow: WindowProvider
where
    Self::Handle: GPUSurfaceHandle,
{
}

/// A [`WindowProvider`] that can drive the application's main loop.
///
/// The `run` method blocks (or hands off control to the OS event loop) and
/// calls `on_frame` once per frame.
pub trait WindowRunner: WindowProvider {
    /// Start the event loop, calling `on_frame` each time a new frame should
    /// be rendered.
    fn run(self, on_frame: impl FnMut() + 'static);
}

/// Resource inserted by [`WindowPlugin`](crate::rendering::window_plugin::WindowPlugin)
/// that gives systems access to the window handle and the exposed value.
pub struct WindowResource<W: WindowProvider> {
    /// The raw window handle, used for surface creation and size queries.
    pub handle: W::Handle,
    /// The platform-specific exposed value (e.g. `Arc<winit::window::Window>`).
    pub exposed: W::Exposed,
}
