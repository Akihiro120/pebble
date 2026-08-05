# Apps and Plugins

`App` owns everything: the ECS world, resources, the registered systems, and the runner that drives the main loop. You build one by chaining calls, then hand it off:

```rust
use pebble::prelude::*;

fn main() {
    App::new()
        .add_plugin(MyWindowPlugin)
        .add_plugin(MyBackendPlugin)
        .add_plugin(MyGamePlugin)
        .build()
        .run();
}
```

## Plugins are the unit of composition

A `Plugin` is anything implementing one method:

```rust
pub trait Plugin {
    fn build(&self, app: &mut App);
}
```

That's the entire extension point. Windowing, the graphics backend, every asset type, and your own game-specific setup are all just plugins. `build` receives `&mut App` and can add resources, register systems, or queue further plugins — plugins can register other plugins, and `App::build()` keeps draining the queue (up to a hard limit of 64 passes, to catch an accidental registration cycle) until nothing new shows up.

Here's a minimal one, from the `orbit_camera` example:

```rust
struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Time {
            time: Instant::now(),
            last_time: Instant::now(),
            delta_time: 0.0,
        })
        .add_system(SystemStage::PreUpdate, update_delta_time);
    }
}
```

Nothing here is special-cased by the framework — `TimePlugin` uses exactly the two calls (`add_resource`, `add_system`) available to any other code with a `&mut App`. Writing your own plugins is how you organize a real project: one plugin per subsystem (input, camera, physics, UI), each self-contained, added to `App` in `main` in whatever order makes sense.

## What `build()` actually does

Three things, in order:

1. Runs every queued plugin's `build`, repeating until no plugin queues another.
2. **Validates resource requirements.** Every system's declared dependencies (see [Resources](./resources.md)) are checked against what plugins have declared they'll eventually provide. A system requiring a resource that nothing will ever insert is a near-certain bug — `build()` panics immediately, naming every offending system and resource, rather than letting it surface as a runtime panic several ticks into `run()`.
3. **Settles the asset pipeline as far as it can go synchronously** (see [The Asset Pipeline and Handles](./the-asset-pipeline.md)) — so resources that don't need to wait for anything asynchronous (a GPU backend arriving on another thread, say) are ready the moment `build()` returns, not one tick later.

`run()` is deliberately thin: it hands `self` to whatever runner is currently installed and does nothing else. The default runner just loops `app.update()` forever; a window plugin normally replaces it with one that also pumps the OS event loop (see [Windows and Backends](./windows-and-backends.md)).

## There is no "Startup" stage

Frameworks with a fixed set of lifecycle stages usually have a dedicated `Startup` one. Pebble doesn't — anything that should run once is an ordinary system wrapped in `.once()`, covered in [Systems and Stages](./systems-and-stages.md#run-once). This keeps the mental model to one thing ("systems run on stages, every tick") instead of two ("systems run on stages, except the ones that only run at the start, which are different").
