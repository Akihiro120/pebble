pub struct InitSender<T>(oneshot::Sender<T>);

impl<T: 'static + Send> InitSender<T> {
    pub fn send(self, value: T) {
        if self.0.send(value).is_err() {
            tracing::warn!("GPU Backend finished init but receiver was dropped");
        }
    }
}

pub struct InitReceiver<T>(oneshot::Receiver<T>);

impl<T: 'static + Send> InitReceiver<T> {
    pub fn try_recv(&mut self) -> Result<T, oneshot::TryRecvError> {
        self.0.try_recv()
    }
}

pub fn init_channel<T: 'static + Send>() -> (InitSender<T>, InitReceiver<T>) {
    let (tx, rx) = oneshot::channel();
    (InitSender(tx), InitReceiver(rx))
}
