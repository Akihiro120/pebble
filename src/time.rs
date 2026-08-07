//! Frame timing — `TimePlugin` inserts [`Time`] as a resource and ticks it
//! once per frame in `PreUpdate`, before any gameplay system runs.

use std::time::Duration;
// `web_time::Instant`, not `std::time::Instant`: the latter's `wasm32`
// support depends on how the final binary is linked (some setups panic
// on `Instant::now()`), whereas `web_time` always calls
// `performance.now()` directly. Identical API, backed by
// `std::time::Instant` itself on every other target.
use web_time::Instant;

use crate::{
    app::SystemStage,
    ecs::{plugin::Plugin, system::ResMut},
};

/// This tick's frame timing. Cheap to read every system that needs it —
/// `delta_seconds`/`elapsed_seconds` are plain `f32`, no locking.
pub struct Time {
    start: Instant,
    last_tick: Instant,
    delta: Duration,
    elapsed: Duration,
}

impl Time {
    fn new() -> Self {
        let now = Instant::now();
        Self { start: now, last_tick: now, delta: Duration::ZERO, elapsed: Duration::ZERO }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.delta = now.duration_since(self.last_tick);
        self.last_tick = now;
        self.elapsed = now.duration_since(self.start);
    }

    /// Time since the previous tick.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// [`Time::delta`] as seconds — the number to multiply a per-second
    /// rate by (`transform.x += speed * time.delta_seconds()`).
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Time since `TimePlugin` was built (app startup), as of this tick.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// [`Time::elapsed`] as seconds.
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    /// `1.0 / delta_seconds()` — this tick's instantaneous frame rate.
    /// `0.0` on the very first tick (`delta` is still zero) rather than
    /// dividing by zero. Jitters frame to frame same as `delta` itself;
    /// average it yourself over a window if you want a smoothed display
    /// value.
    pub fn fps(&self) -> f32 {
        let seconds = self.delta_seconds();
        if seconds > 0.0 { 1.0 / seconds } else { 0.0 }
    }
}

fn tick_time(mut time: ResMut<Time>) {
    time.tick();
}

/// Registers [`Time`] as a resource and advances it once per frame.
///
/// `App::new()` already builds this in, so `Res<Time>` works out of the
/// box — you don't need to `add_plugin(TimePlugin)` yourself. It's still a
/// public plugin (rather than baking the resource/system straight into
/// `App::new()`) for the rare case of assembling an `App` by some other
/// path that skips `App::new()`. Idempotent either way: it only inserts
/// [`Time`]/registers its tick system the first time it actually runs, so
/// an explicit `add_plugin(TimePlugin)` alongside the automatic one is
/// harmless rather than double-ticking.
///
/// Backend-agnostic — works the same with `pebble::wgpu` or a hand-rolled
/// `Backend`, and even with no graphics backend at all (see `ecs_basics`).
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        if app.try_insert_resource(Time::new()) {
            app.add_system(SystemStage::PreUpdate, tick_time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_time_has_zero_delta_and_elapsed() {
        let time = Time::new();
        assert_eq!(time.delta(), Duration::ZERO);
        assert_eq!(time.elapsed(), Duration::ZERO);
        assert_eq!(time.fps(), 0.0);
    }

    #[test]
    fn ticking_advances_delta_and_accumulates_elapsed() {
        let mut time = Time::new();
        std::thread::sleep(Duration::from_millis(5));
        time.tick();

        assert!(time.delta_seconds() > 0.0);
        assert!(time.elapsed_seconds() >= time.delta_seconds());
        assert!(time.fps() > 0.0);

        let first_elapsed = time.elapsed();
        std::thread::sleep(Duration::from_millis(5));
        time.tick();
        assert!(time.elapsed() > first_elapsed);
    }
}
