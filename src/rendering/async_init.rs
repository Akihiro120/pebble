use crate::rendering::{backend::Backend, sync::InitReceiver};

/// Holds the [`InitReceiver`] while waiting for an async backend to finish
/// initialising. Removed from the world once the backend arrives.
pub(crate) struct PendingBackend<B: Backend> {
    pub(crate) receiver: std::sync::Mutex<InitReceiver<B>>,
}
