//! A small, fixed-size worker pool for offloading CPU-bound work off the
//! main thread — mip/image processing, physics steps, any one-off or
//! recurring task that shouldn't block a frame.
//!
//! Deliberately NOT a full async runtime: no cancellation, no priorities,
//! no work-stealing. Just a bounded number of OS threads pulling jobs off
//! one shared, lock-free MPMC queue, with results delivered back via a
//! channel you poll from an ordinary system. If you outgrow this — need
//! cancellation, need priority scheduling — that's real, separate
//! infrastructure to build once you have a concrete case for it, not
//! something to guess at now.
//!
//! Requires the `crossbeam-channel` crate (lock-free MPMC), since
//! `std::sync::mpsc` only supports a single consumer and would otherwise
//! force a `Mutex` around the receiver for multiple worker threads.

use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender, unbounded};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use crate::ecs::plugin::Plugin;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Extracts a human-readable message from a `catch_unwind` payload — panics
/// via `panic!("...")`/`.unwrap()`/`.expect("...")` all land in one of these
/// two downcasts; anything else (a panic with a non-`&str`/`String` payload,
/// via `std::panic::panic_any`) falls back to a generic label rather than
/// failing to report anything at all.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<panic payload was not a string>".to_string()
    }
}

/// The bound a future must satisfy to be handed to
/// [`BackgroundTasks::spawn_async`] — mirrored as a trait (rather than
/// written out at every call site) so both `spawn_async` itself and the
/// scheduler's [`AsyncExt::detach`](crate::ecs::system::AsyncExt::detach)/
/// [`AsyncEventWriter`](crate::ecs::events::AsyncEventWriter) share one definition
/// instead of duplicating the native/web split.
///
/// Native futures cross to a worker thread, so they must be `Send`; web
/// futures run on the browser's microtask queue on the same thread, so they
/// don't need to be — which is what lets a web future capture `!Send`
/// browser-bound types (`JsValue`, a `web_sys` handle, ...).
#[cfg(not(target_arch = "wasm32"))]
pub trait SpawnableFuture<T>: std::future::Future<Output = T> + Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T, F: std::future::Future<Output = T> + Send + 'static> SpawnableFuture<T> for F {}

#[cfg(target_arch = "wasm32")]
pub trait SpawnableFuture<T>: std::future::Future<Output = T> + 'static {}
#[cfg(target_arch = "wasm32")]
impl<T, F: std::future::Future<Output = T> + 'static> SpawnableFuture<T> for F {}

/// The worker pool itself. Insert as a resource once, at startup; every
/// system that needs to offload work reaches for `Res<BackgroundTasks>`
/// and calls `spawn`.
///
/// Cheap to clone (an internal channel sender) — clone it out of a `Res`
/// borrow to move an owned handle into a detached future, e.g. from a
/// system registered with [`AsyncExt::detach`](crate::ecs::system::AsyncExt::detach).
#[derive(Clone)]
pub struct BackgroundTasks {
    job_tx: CbSender<Job>,
}

impl BackgroundTasks {
    /// Spawns `worker_count` OS threads, each pulling jobs off one shared,
    /// lock-free queue until the pool itself is dropped. A worker count
    /// around your CPU's core count (minus one, to leave room for the
    /// main thread) is a reasonable default; tune based on actual
    /// measured load.
    ///
    /// On `wasm32` there are no OS threads to spawn, so `worker_count` is
    /// ignored and [`spawn_blocking`](Self::spawn_blocking) queues jobs that never run —
    /// [`spawn_async`](Self::spawn_async) is the one that's web-compatible,
    /// since it drives the browser's microtask queue via
    /// `wasm_bindgen_futures::spawn_local` instead of a worker thread.
    pub fn new(worker_count: usize) -> Self {
        let (job_tx, job_rx): (CbSender<Job>, CbReceiver<Job>) = unbounded();

        #[cfg(not(target_arch = "wasm32"))]
        for _ in 0..worker_count.max(1) {
            let job_rx = job_rx.clone(); // cheap — crossbeam receivers are natively Clone, no Mutex needed
            std::thread::spawn(move || {
                // `recv()` blocks this worker thread only, until a job
                // arrives or every sender (the pool, plus any clones) is
                // dropped — no lock contention between workers picking up
                // jobs concurrently.
                while let Ok(job) = job_rx.recv() {
                    // `spawn_blocking`/`spawn_async` already catch_unwind
                    // around the caller's closure/future, so a panic
                    // reaching here at all means something upstream failed
                    // to report it through its own TaskHandle — this is a
                    // last-resort net so *that* doesn't also cost the pool
                    // a worker thread permanently. Every worker dying one
                    // panic at a time, with nothing ever telling the app
                    // its background work silently stopped happening, is
                    // exactly the failure mode this whole module exists to
                    // avoid.
                    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
                        tracing::error!(
                            "BackgroundTasks: a job panicked without going through its own \
                             error reporting — the worker thread survived regardless: {}",
                            panic_message(payload)
                        );
                    }
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (worker_count, &job_rx);

        Self { job_tx }
    }

    /// Queue `work` to run on the pool. Returns a [`TaskHandle`] you can
    /// poll (non-blocking) from any system to check whether it's done.
    ///
    /// `work` runs on whichever worker thread picks it up next — don't
    /// assume anything about timing or ordering relative to other spawned
    /// tasks unless you build that coordination yourself.
    ///
    /// **Native-only.** There are no OS threads to block on in a browser
    /// tab, so on `wasm32` this queues a job that never runs — use
    /// [`spawn_async`](Self::spawn_async) instead, which works on both.
    /// The `_blocking` suffix names what makes this one platform-specific:
    /// it occupies its worker thread for as long as `work` runs, same as
    /// `std::thread::spawn` would.
    pub fn spawn_blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> TaskHandle<T> {
        let (result_tx, result_rx) = channel::<Result<T, String>>();
        let job: Job = Box::new(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).map_err(|payload| {
                let message = panic_message(payload);
                tracing::error!("BackgroundTasks: a spawned task panicked: {message}");
                message
            });
            // ignore: receiver may have been dropped (TaskHandle discarded
            // by the caller, e.g. `.detach()`), that's fine either way.
            let _ = result_tx.send(outcome);
        });
        // If this fails, every worker thread has panicked and the pool is
        // effectively dead — surfaced through `TaskHandle::poll` as
        // `Panicked` (the disconnected-channel case), same as any other
        // task whose sender never got to send, rather than panicking here
        // and crashing an unrelated caller trying to queue new work.
        let _ = self.job_tx.send(job);
        TaskHandle { rx: result_rx }
    }

    /// Queue an already-constructed `future` to run to completion, and
    /// return a [`TaskHandle`] you can poll for its result.
    ///
    /// This is the primitive [`AsyncExt::detach`](crate::ecs::system::AsyncExt::detach)
    /// uses under the hood; call it directly instead when you want the
    /// `TaskHandle` back to poll for a result, rather than firing the
    /// future off and forgetting it.
    ///
    /// - **Native**: blocks whichever worker thread picks it up (via
    ///   [`pollster::block_on`]) for as long as the future takes to
    ///   resolve — there's no cooperative multitasking between futures
    ///   sharing a worker, so a slow future occupies that worker
    ///   exclusively, same as a slow [`spawn_blocking`](Self::spawn_blocking)
    ///   closure would. The future must be `Send` to cross to that worker thread.
    /// - **Web**: runs on the browser's microtask queue via
    ///   `wasm_bindgen_futures::spawn_local`, on the same (only) thread —
    ///   so it does *not* need to be `Send`, which is what lets it capture
    ///   `!Send` browser-bound types (`JsValue`, a `web_sys` handle, ...).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_async<T: Send + 'static>(
        &self,
        future: impl SpawnableFuture<T>,
    ) -> TaskHandle<T> {
        self.spawn_blocking(move || pollster::block_on(future))
    }

    /// See the native [`spawn_async`](Self::spawn_async) docs above for the
    /// full contract — this is the web counterpart, driven by the
    /// browser's microtask queue instead of a worker thread.
    #[cfg(target_arch = "wasm32")]
    pub fn spawn_async<T: 'static>(&self, future: impl SpawnableFuture<T>) -> TaskHandle<T> {
        let (result_tx, result_rx) = channel::<Result<T, String>>();
        wasm_bindgen_futures::spawn_local(async move {
            // No native `catch_unwind` wrapper here — catching a panic
            // across an `.await` point needs a polling combinator this
            // module doesn't currently pull in a dependency for (native's
            // `spawn_blocking`/`spawn_async` can wrap synchronously instead,
            // which is why only this platform lacks it). If `future` panics,
            // `result_tx` is simply never sent to; `TaskHandle::poll` still
            // reports that as `Panicked` once the disconnected channel is
            // observed, just without a captured message — install a wasm
            // panic hook (`console_error_panic_hook`) to see the message
            // itself in the browser console instead.
            let result = future.await;
            let _ = result_tx.send(Ok(result)); // ignore: receiver may have been dropped, that's fine
        });
        TaskHandle { rx: result_rx }
    }
}

/// The outcome of polling a [`TaskHandle`].
pub enum TaskStatus<T> {
    /// Not finished yet — poll again next tick.
    Pending,
    /// Finished successfully.
    Ready(T),
    /// The task panicked (or, for a [`spawn_blocking`](BackgroundTasks::spawn_blocking)/
    /// [`spawn_async`](BackgroundTasks::spawn_async) task specifically, the
    /// whole worker pool has died) before producing a result — it never
    /// will now. The message is the panic payload where one could be
    /// captured; native tasks always get one, since `spawn_blocking` wraps
    /// the closure in `catch_unwind` directly. A web [`spawn_async`](BackgroundTasks::spawn_async)
    /// task can't be wrapped the same way (see that method's docs), so its
    /// message is a generic placeholder — the real panic message goes to
    /// the browser console instead, via whatever wasm panic hook is
    /// installed.
    Panicked(String),
}

/// A handle to a single in-flight (or already-finished) task.
pub struct TaskHandle<T> {
    rx: Receiver<Result<T, String>>,
}

impl<T> TaskHandle<T> {
    /// Poll for this task's outcome. Never blocks — safe to call every tick.
    ///
    /// Distinguishes "still running" from "panicked and will never produce
    /// a result" — the two states [`try_recv`](Self::try_recv) alone can't
    /// tell apart, since both look like `None` there. Reach for this
    /// instead of `try_recv` whenever a task not finishing is something
    /// your own code should react to, rather than silently wait on forever.
    pub fn poll(&mut self) -> TaskStatus<T> {
        match self.rx.try_recv() {
            Ok(Ok(value)) => TaskStatus::Ready(value),
            Ok(Err(message)) => TaskStatus::Panicked(message),
            Err(TryRecvError::Empty) => TaskStatus::Pending,
            Err(TryRecvError::Disconnected) => TaskStatus::Panicked(
                "the task's sender was dropped without ever sending a result — on native, this \
                 also means the panic that caused it was already logged via tracing::error! at \
                 the time it happened"
                    .to_string(),
            ),
        }
    }

    /// Returns `Some(result)` once the task has finished successfully,
    /// `None` otherwise — whether it's still running or it panicked. Kept
    /// for the common case where you only care *whether* a result showed
    /// up, not why one hasn't; see [`poll`](Self::poll) to tell a panic
    /// apart from ordinary pending.
    pub fn try_recv(&mut self) -> Option<T> {
        match self.poll() {
            TaskStatus::Ready(value) => Some(value),
            TaskStatus::Pending | TaskStatus::Panicked(_) => None,
        }
    }
}

/// Registers `BackgroundTasks` as a resource with the given worker count.
///
/// ```ignore
/// app.add_plugin(BackgroundTasksPlugin::new(4));
/// ```
pub struct BackgroundTasksPlugin {
    worker_count: usize,
}

impl BackgroundTasksPlugin {
    pub fn new(worker_count: usize) -> Self {
        Self { worker_count }
    }
}

impl Plugin for BackgroundTasksPlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_resource(BackgroundTasks::new(self.worker_count));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn poll_until<T>(handle: &mut TaskHandle<T>, timeout: Duration) -> TaskStatus<T> {
        let deadline = Instant::now() + timeout;
        loop {
            match handle.poll() {
                TaskStatus::Pending => {
                    assert!(Instant::now() < deadline, "task did not resolve within {timeout:?}");
                    std::thread::sleep(Duration::from_millis(5));
                }
                status => return status,
            }
        }
    }

    #[test]
    fn a_panicking_task_reports_panicked_instead_of_hanging_forever() {
        // Silence the default panic hook for this test only — we're
        // deliberately panicking a worker thread and don't want a scary
        // backtrace in otherwise-passing test output; catch_unwind doesn't
        // suppress the hook on its own.
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let pool = BackgroundTasks::new(1);
        let mut handle = pool.spawn_blocking(|| -> u32 { panic!("deliberate test panic") });
        let status = poll_until(&mut handle, Duration::from_secs(2));

        std::panic::set_hook(previous_hook);

        match status {
            TaskStatus::Panicked(message) => assert!(message.contains("deliberate test panic")),
            _ => panic!("expected TaskStatus::Panicked"),
        }
    }

    #[test]
    fn the_worker_pool_survives_a_panic_and_keeps_processing_later_tasks() {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        // Exactly one worker — proves that specific thread survived the
        // panic and picked the next job back up, rather than a second
        // worker happening to cover for a dead first one.
        let pool = BackgroundTasks::new(1);
        let mut doomed = pool.spawn_blocking(|| -> u32 { panic!("first task panics") });
        poll_until(&mut doomed, Duration::from_secs(2));

        let mut healthy = pool.spawn_blocking(|| 42u32);
        let status = poll_until(&mut healthy, Duration::from_secs(2));

        std::panic::set_hook(previous_hook);

        match status {
            TaskStatus::Ready(value) => assert_eq!(value, 42),
            _ => panic!("expected the pool's sole worker thread to still be alive and processing"),
        }
    }
}
