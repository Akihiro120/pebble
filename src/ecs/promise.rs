/// The result of polling a [`Promise<T>`].
pub enum PromiseState<T> {
    /// Not resolved yet — poll again next tick.
    Pending,
    /// Resolved. Only returned once, ever.
    Ready(T),
    /// The matching [`Fulfiller`] was dropped without fulfilling — this
    /// promise will never resolve.
    Disconnected,
}

/// A one-off async result you poll each tick — e.g. GPU backend
/// acquisition, [`Buffer::read`](crate::graphics::pipeline::buffers::Buffer::read).
/// Not a resource, not registered anywhere — a plain value you store
/// wherever fits (a [`Local`](crate::ecs::local::Local), a field on your
/// own resource/component).
pub struct Promise<T> {
    rx: oneshot::Receiver<T>,
}

// oneshot::Receiver<T> is Send (given T: Send) but not Sync — it uses a raw
// pointer internally and only guarantees safety for a single consumer, not
// concurrent access through a shared reference. This engine runs systems
// one at a time on a single thread, so a Promise is never actually touched
// concurrently. Needed so Promise<T> can be stored in a Local<T>/resource,
// both of which require Send + Sync.
unsafe impl<T> Sync for Promise<T> {}

impl<T> Promise<T> {
    /// Creates a paired [`Fulfiller<T>`]/`Promise<T>` — whoever produces
    /// the value calls `fulfiller.fulfill(value)`, whoever needs it polls
    /// the `Promise` each tick.
    pub fn new() -> (Fulfiller<T>, Promise<T>) {
        let (tx, rx) = oneshot::channel();
        (Fulfiller { tx }, Promise { rx })
    }

    /// Checks whether this has resolved yet. Non-blocking, safe to call
    /// every tick.
    pub fn poll(&self) -> PromiseState<T> {
        match self.rx.try_recv() {
            Ok(value) => PromiseState::Ready(value),
            Err(oneshot::TryRecvError::Empty) => PromiseState::Pending,
            Err(oneshot::TryRecvError::Disconnected) => PromiseState::Disconnected,
        }
    }
}

/// The producing half of a [`Promise`], from [`Promise::new`].
pub struct Fulfiller<T> {
    tx: oneshot::Sender<T>,
}

impl<T> Fulfiller<T> {
    /// Resolves the matching `Promise` with `value`.
    pub fn fulfill(self, value: T) {
        let _ = self.tx.send(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_is_pending_before_a_fulfill_and_ready_after() {
        let (fulfiller, promise) = Promise::new();

        assert!(matches!(promise.poll(), PromiseState::Pending));

        fulfiller.fulfill(42);

        assert!(matches!(promise.poll(), PromiseState::Ready(42)));
    }

    #[test]
    fn poll_is_disconnected_once_the_fulfiller_is_dropped_without_fulfilling() {
        let (fulfiller, promise) = Promise::<i32>::new();

        drop(fulfiller);

        assert!(matches!(promise.poll(), PromiseState::Disconnected));
    }
}
