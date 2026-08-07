# Time

`TimePlugin` inserts `Time` as a resource and ticks it once per frame, in `PreUpdate`, before any gameplay system runs:

```rust
use pebble::prelude::*;

app.add_plugin(TimePlugin);

fn movement(time: Res<Time>, mut query: Query<&mut Position>) {
    for pos in query.iter() {
        pos.x += 3.0 * time.delta_seconds(); // move at 3 units/second
    }
}
```

- `delta_seconds()` / `delta()` — time since the previous tick, as `f32` seconds or a `Duration`. Multiply a per-second rate by this to get a frame-independent step.
- `elapsed_seconds()` / `elapsed()` — time since `TimePlugin` was built (app startup).
- `fps()` — `1.0 / delta_seconds()`, this tick's instantaneous frame rate. `0.0` on the first tick rather than dividing by zero. It jitters same as `delta` does; average it yourself over a window (e.g. a small ring buffer in a `Local<T>`) if you want a smoothed display value.

## Backend-agnostic

`Time` doesn't read from `Input`, the window, or any graphics backend — it measures wall-clock time directly (via `web_time::Instant`, which works correctly on `wasm32` as well as native, unlike `std::time::Instant`). That means `TimePlugin` works identically with `pebble::wgpu`, a hand-rolled `Backend`, or no graphics backend at all — `examples/ecs_basics` could use it even though it never opens a window.

This is also why `Time` and [`Input`](./input.md) are separate resources even though `Input` happens to expose its own `delta_time()`: `Input` only exists once a window/backend inserts it, and only ever inside `pebble::wgpu`'s winit-backed `Input` specifically. `Time` exists as soon as you register `TimePlugin`, regardless of what (if anything) else is in the app.
