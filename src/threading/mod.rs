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
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::ecs::plugin::Plugin;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// The worker pool itself. Insert as a resource once, at startup; every
/// system that needs to offload work reaches for `Res<BackgroundTasks>`
/// and calls `spawn`.
pub struct BackgroundTasks {
    job_tx: CbSender<Job>,
}

impl BackgroundTasks {
    /// Spawns `worker_count` OS threads, each pulling jobs off one shared,
    /// lock-free queue until the pool itself is dropped. A worker count
    /// around your CPU's core count (minus one, to leave room for the
    /// main thread) is a reasonable default; tune based on actual
    /// measured load.
    pub fn new(worker_count: usize) -> Self {
        let (job_tx, job_rx): (CbSender<Job>, CbReceiver<Job>) = unbounded();

        for _ in 0..worker_count.max(1) {
            let job_rx = job_rx.clone(); // cheap — crossbeam receivers are natively Clone, no Mutex needed
            std::thread::spawn(move || {
                // `recv()` blocks this worker thread only, until a job
                // arrives or every sender (the pool, plus any clones) is
                // dropped — no lock contention between workers picking up
                // jobs concurrently.
                while let Ok(job) = job_rx.recv() {
                    job();
                }
            });
        }

        Self { job_tx }
    }

    /// Queue `work` to run on the pool. Returns a [`TaskHandle`] you can
    /// poll (non-blocking) from any system to check whether it's done.
    ///
    /// `work` runs on whichever worker thread picks it up next — don't
    /// assume anything about timing or ordering relative to other spawned
    /// tasks unless you build that coordination yourself.
    pub fn spawn<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> TaskHandle<T> {
        let (result_tx, result_rx) = channel::<T>();
        let job: Job = Box::new(move || {
            let result = work();
            let _ = result_tx.send(result); // ignore: receiver may have been dropped, that's fine
        });
        // If this fails, every worker thread has panicked and the pool is
        // effectively dead — surfaced as a permanently-pending TaskHandle
        // rather than a panic here, since a dead pool shouldn't crash an
        // unrelated caller trying to queue new work.
        let _ = self.job_tx.send(job);
        TaskHandle { rx: result_rx }
    }
}

/// A handle to a single in-flight (or already-finished) task. Poll it
/// from an ordinary system with [`try_recv`](TaskHandle::try_recv) —
/// never blocks.
pub struct TaskHandle<T> {
    rx: Receiver<T>,
}

impl<T> TaskHandle<T> {
    /// Returns `Some(result)` once the task has finished, `None`
    /// otherwise. Never blocks — safe to call every tick.
    pub fn try_recv(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
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
