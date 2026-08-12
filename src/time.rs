use std::time::Duration;
use web_time::Instant;

use crate::{
    ecs::{plugin::Plugin, resources::Write, system::SystemStage},
};

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

    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn fps(&self) -> f32 {
        let seconds = self.delta_seconds();
        if seconds > 0.0 { 1.0 / seconds } else { 0.0 }
    }
}

fn tick_time(mut time: Write<Time>) {
    time.tick();
}

pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.insert_resource(Time::new()).add_system(SystemStage::PreUpdate, tick_time)
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
