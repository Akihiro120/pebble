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

## Plugins composing other plugins

"Adds further plugins of its own" is how pebble's own `GraphicsPlugin` is actually built — it's not a special case, just three smaller plugins bundled behind one name:

```rust,ignore
pub struct GraphicsPlugin;
impl Plugin for GraphicsPlugin {
    fn build(self, app: App) -> App {
        app.add_plugin(WindowPlugin::default())
            .add_plugin(BackendPlugin)
            .add_plugin(BuiltinAssetsPlugin)
    }
}
```

Your own plugins can do the same — group a handful of related plugins/systems your project always wants together under one name, so call sites stay a one-liner instead of repeating the same five `.add_plugin(...)` calls in every example/binary.

A closure is the lighter-weight version of this for one-off, non-reusable setup — useful for something conditional you'd otherwise have to hand-roll a whole struct for:

```rust,ignore
fn debug_plugin(app: App) -> App {
    app.add_system(SystemStage::PostRender, print_frame_time)
}

let app = App::new().add_plugin(GraphicsPlugin);
let app = if cfg!(debug_assertions) { app.add_plugin(debug_plugin) } else { app };
```

## Key methods

- `insert_resource<T>`/`remove_resource<T>` — see [Resources](./resources.md).
- `add_plugin<P: Plugin>` — runs the plugin's `build`.
- `add_system(stage, system)` — see [Systems and Stages](./systems-and-stages.md).
- `add_event::<T>()` — see [Events](./events.md).
- `add_observer(fn)` — see [Observers](./observers.md).
- `set_runner(fn)` — overrides how the main loop is driven; `WindowPlugin` uses this to hand control to `winit`'s event loop instead of the default headless polling loop.
- `with_logging()` — initializes `tracing_subscriber` so `tracing::info!`/`warn!`/`error!` calls throughout the engine actually print.
- `run()` — consumes the app and starts it.
