use crate::{
    prelude::{Backend, GPUSurfaceHandle},
    rendering::sync::{InitReceiver, InitSender},
};

/// Extension of [`Backend`] for backends that initialise asynchronously.
///
/// # Deprecated
/// The [`Backend`] trait now unifies native and web initialisation via its
/// [`init`](Backend::init) method and the [`InitSender`] channel. Prefer
/// implementing `Backend::init` directly; this trait is kept for compatibility
/// and may be removed in a future release.
#[deprecated = "Backend::init now handles both native and web; implement that directly instead"]
pub trait AsyncInit: Backend {
    /// Begin async initialisation. Deliver the finished backend through `sender`.
    fn init_async(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>);
}

/// Holds the [`InitReceiver`] while waiting for an async backend to finish
/// initialising. Removed from the world once the backend arrives.
pub(crate) struct PendingBackend<B: Backend> {
    pub(crate) receiver: std::sync::Mutex<InitReceiver<B>>,
}
