pub enum PromiseState<T> {
    Pending,
    Ready(T),
    Disconnected,
}

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
    // paired with a Fulfiller<T> — nothing outside this module needs to
    // know the `oneshot` crate exists
    pub fn new() -> (Fulfiller<T>, Promise<T>) {
        let (tx, rx) = oneshot::channel();
        (Fulfiller { tx }, Promise { rx })
    }

    // non-blocking; safe to call every tick. Once this returns `Ready` it
    // will never return `Ready` again (the underlying oneshot is spent) —
    // that's a property of the type this wraps, not something to check for
    // here.
    pub fn poll(&self) -> PromiseState<T> {
        match self.rx.try_recv() {
            Ok(value) => PromiseState::Ready(value),
            Err(oneshot::TryRecvError::Empty) => PromiseState::Pending,
            Err(oneshot::TryRecvError::Disconnected) => PromiseState::Disconnected,
        }
    }
}

pub struct Fulfiller<T> {
    tx: oneshot::Sender<T>,
}

impl<T> Fulfiller<T> {
    // ignores send failure: it only happens if the matching Promise was
    // dropped, meaning nothing is left to deliver `value` to
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
