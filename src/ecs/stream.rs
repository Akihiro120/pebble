use std::sync::mpsc;

/// The result of polling a [`Stream<T>`].
pub enum StreamState<T> {
    /// Nothing waiting right now — poll again next tick.
    Pending,
    /// The next value, in the order it was emitted.
    Ready(T),
    /// Every [`Emitter`] is gone and the backlog is drained — this stream
    /// will never yield again.
    Disconnected,
}

/// The multi-shot sibling of [`Promise`](crate::ecs::promise::Promise): an
/// open-ended sequence of async values you poll each tick — chunks arriving
/// over a websocket, messages from a background thread, frames from a
/// worker. Not a resource, not registered anywhere — a plain value you
/// store wherever fits (a [`Local`](crate::ecs::local::Local), a field on
/// your own resource/component).
///
/// ```ignore
/// let (emitter, stream) = Stream::new();
/// // hand `emitter` to the producer — it's Clone + Send, so it can live
/// // in a websocket callback, another thread, or an async task:
/// emitter.emit(chunk);
/// // ...and drain the stream from a system, each tick:
/// while let StreamState::Ready(chunk) = stream.poll() {
///     /* ... */
/// }
/// ```
pub struct Stream<T> {
    rx: mpsc::Receiver<T>,
}

// mpsc::Receiver<T> is Send (given T: Send) but not Sync — it only
// guarantees safety for a single consumer, not concurrent access through a
// shared reference. This engine runs systems one at a time on a single
// thread, so a Stream is never actually touched concurrently. Needed so
// Stream<T> can be stored in a Local<T>/resource, both of which require
// Send + Sync.
unsafe impl<T> Sync for Stream<T> {}

impl<T> Stream<T> {
    /// Creates a paired [`Emitter<T>`]/`Stream<T>` — whoever produces
    /// values calls `emitter.emit(value)` as often as it likes, whoever
    /// consumes them polls the `Stream` each tick.
    pub fn new() -> (Emitter<T>, Stream<T>) {
        let (tx, rx) = mpsc::channel();
        (Emitter { tx }, Stream { rx })
    }

    /// Takes the next emitted value, if one is waiting. Non-blocking, safe
    /// to call every tick — drain a burst with
    /// `while let StreamState::Ready(v) = stream.poll()`.
    pub fn poll(&self) -> StreamState<T> {
        match self.rx.try_recv() {
            Ok(value) => StreamState::Ready(value),
            Err(mpsc::TryRecvError::Empty) => StreamState::Pending,
            Err(mpsc::TryRecvError::Disconnected) => StreamState::Disconnected,
        }
    }
}

/// The producing half of a [`Stream`], from [`Stream::new`]. Cloneable —
/// every clone feeds the same stream.
pub struct Emitter<T> {
    tx: mpsc::Sender<T>,
}

// Manual impl instead of derive: a derived Clone would demand T: Clone,
// but cloning the sending half never clones a T.
impl<T> Clone for Emitter<T> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

impl<T> Emitter<T> {
    /// Sends `value` to the matching `Stream`. Values arrive in emit
    /// order. Quietly does nothing if the `Stream` has been dropped.
    pub fn emit(&self, value: T) {
        let _ = self.tx.send(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_is_pending_before_an_emit_and_ready_after() {
        let (emitter, stream) = Stream::new();

        assert!(matches!(stream.poll(), StreamState::Pending));

        emitter.emit(42);

        assert!(matches!(stream.poll(), StreamState::Ready(42)));
        assert!(matches!(stream.poll(), StreamState::Pending));
    }

    #[test]
    fn polls_yield_every_emitted_value_in_emit_order() {
        let (emitter, stream) = Stream::new();

        emitter.emit(1);
        emitter.emit(2);
        emitter.emit(3);

        let mut drained = Vec::new();
        while let StreamState::Ready(value) = stream.poll() {
            drained.push(value);
        }
        assert_eq!(drained, vec![1, 2, 3]);
    }

    #[test]
    fn a_cloned_emitter_feeds_the_same_stream() {
        let (emitter, stream) = Stream::new();
        let clone = emitter.clone();

        emitter.emit(1);
        clone.emit(2);

        assert!(matches!(stream.poll(), StreamState::Ready(1)));
        assert!(matches!(stream.poll(), StreamState::Ready(2)));
    }

    #[test]
    fn poll_is_disconnected_only_after_every_emitter_is_gone_and_the_backlog_is_drained() {
        let (emitter, stream) = Stream::new();
        let clone = emitter.clone();

        emitter.emit(1);
        drop(emitter);

        // one emitter left — still just Pending after the backlog
        assert!(matches!(stream.poll(), StreamState::Ready(1)));
        assert!(matches!(stream.poll(), StreamState::Pending));

        clone.emit(2);
        drop(clone);

        // last emitter gone — the buffered value still arrives first
        assert!(matches!(stream.poll(), StreamState::Ready(2)));
        assert!(matches!(stream.poll(), StreamState::Disconnected));
    }

    #[test]
    fn an_emitter_works_from_another_thread() {
        let (emitter, stream) = Stream::new();

        std::thread::spawn(move || emitter.emit(7)).join().unwrap();

        assert!(matches!(stream.poll(), StreamState::Ready(7)));
    }
}
