/// The sending half of a one-shot backend initialisation channel.
///
/// Pass this to [`Backend::init`](crate::rendering::backend::Backend::init) so
/// the (potentially async) init task can deliver the finished backend.
pub struct InitSender<T>(oneshot::Sender<T>);

impl<T: 'static + Send> InitSender<T> {
    /// Send the completed backend to the waiting receiver.
    ///
    /// Logs a warning if the receiver was already dropped.
    pub fn send(self, value: T) {
        if self.0.send(value).is_err() {
            tracing::warn!("GPU Backend finished init but receiver was dropped");
        }
    }
}

/// The receiving half of a one-shot backend initialisation channel.
pub struct InitReceiver<T>(oneshot::Receiver<T>);

impl<T: 'static + Send> InitReceiver<T> {
    /// Non-blocking poll for the completed backend.
    pub fn try_recv(&mut self) -> Result<T, oneshot::TryRecvError> {
        self.0.try_recv()
    }
}

/// Create a matched [`InitSender`] / [`InitReceiver`] pair for communicating a
/// finished backend from its init task back to the main loop.
pub fn init_channel<T: 'static + Send>() -> (InitSender<T>, InitReceiver<T>) {
    let (tx, rx) = oneshot::channel();
    (InitSender(tx), InitReceiver(rx))
}
