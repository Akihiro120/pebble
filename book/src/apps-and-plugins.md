# Apps and Plugins

`App` is the central object: it owns the ECS world, every resource, and every system, organized into stages. It's built by chaining methods that take `self` by value and return `Self`, so a typical setup reads as one expression:

```rust,ignore
use pebble::app::App;
use pebble::ecs::system::SystemStage;
use pebble::graphics::GraphicsPlugin;

App::new()
    .with_logging()
    .add_plugin(GraphicsPlugin)
    .add_system(SystemStage::Ready, setup)
    .add_system(SystemStage::Update, my_game_logic)
    .run();
```

`App::new()` gives you a completely empty app — no window, no GPU backend, no `Time`. Everything is opt-in via `.add_plugin(...)`.

## Plugins

A `Plugin` is a composable unit of app setup — it inserts resources, registers systems, or adds further plugins:

```rust,ignore
use pebble::app::App;
use pebble::ecs::plugin::Plugin;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(self, app: App) -> App {
        app.insert_resource(MyResource::default())
            .add_system(SystemStage::Update, my_system)
    }
}
```

Any `FnOnce(App) -> App` also implements `Plugin`, so a plain closure works without a named type — handy for one-off setup you don't intend to reuse.

## Key methods

- `insert_resource<T>`/`remove_resource<T>` — see [Resources](./resources.md).
- `add_plugin<P: Plugin>` — runs the plugin's `build`.
- `add_system(stage, system)` — see [Systems and Stages](./systems-and-stages.md).
- `add_event::<T>()` — see [Events](./events.md).
- `add_observer(fn)` — see [Observers](./observers.md).
- `set_runner(fn)` — overrides how the main loop is driven; `WindowPlugin` uses this to hand control to `winit`'s event loop instead of the default headless polling loop.
- `with_logging()` — initializes `tracing_subscriber` so `tracing::info!`/`warn!`/`error!` calls throughout the engine actually print.
- `run()` — consumes the app and starts it.
