//! Bench-only instrumentation hooks (`feature = "bench"`).
//!
//! These exist in the engine because they instrument the one thing an
//! application cannot reach from outside: the window event loop. In keeping
//! with pebble's bring-your-own philosophy, this module holds **no policy
//! and no app contract** — it exposes plain functions; how the knobs get set
//! (a JS global, an env var, a config file) is entirely the app's business.
//!
//! 1. **Fixed-step frame driver** (web): the normal web loop runs exactly one
//!    app update per `RedrawRequested` (i.e. per rAF, vsync/compositor
//!    paced). [`run_frames`] instead runs [`set_steps_per_redraw`]-many
//!    updates back-to-back per redraw, so GPU work is queued as fast as the
//!    GPU can drain it and DVFS never sees an idle gap. All updates in a
//!    batch render into the same canvas texture; the compositor presents
//!    once per batch.
//! 2. **Per-update CPU wall time**: each `on_frame()` (one full ECS tick,
//!    including all queue submits) is timed and retrievable by the app via
//!    [`last_cpu_time`] on the following tick.
//!
//! The frame counter here counts *engine ticks from startup* (including
//! pre-GPU-init ticks), which is why apps should key trajectories etc. on
//! their own rendered-frame counter and only use [`last_cpu_time`]'s frame
//! index for correlation.

use std::cell::Cell;

thread_local! {
    static FRAME: Cell<u64> = const { Cell::new(0) };
    static LAST_CPU: Cell<Option<(u64, f64)>> = const { Cell::new(None) };
    static STEPS: Cell<u32> = const { Cell::new(1) };
}

/// Index of the engine tick currently running (or about to run).
pub fn current_frame() -> u64 {
    FRAME.with(|f| f.get())
}

/// `(tick_index, wall_ms)` of the most recently *completed* app update.
pub fn last_cpu_time() -> Option<(u64, f64)> {
    LAST_CPU.with(|l| l.get())
}

/// Set how many app updates run back-to-back per redraw event (clamped to
/// [1, 64]; default 1 — i.e. the normal loop). Takes effect from the next
/// redraw. Call from a system whenever the app's own source of truth
/// changes; the engine never reads that source itself.
pub fn set_steps_per_redraw(steps: u32) {
    STEPS.with(|s| s.set(steps.clamp(1, 64)));
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    web_sys::window()
        .expect("no window")
        .performance()
        .expect("no performance")
        .now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Run one redraw's worth of app updates (see module docs), timing each.
pub(crate) fn run_frames(on_frame: &mut impl FnMut()) {
    let steps = STEPS.with(|s| s.get());
    for _ in 0..steps {
        let idx = FRAME.with(|f| f.get());
        let t0 = now_ms();
        on_frame();
        let dt = now_ms() - t0;
        LAST_CPU.with(|l| l.set(Some((idx, dt))));
        FRAME.with(|f| f.set(idx + 1));
    }
}
