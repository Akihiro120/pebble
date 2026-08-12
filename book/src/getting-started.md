# Getting Started

Add pebble to your `Cargo.toml`:

```toml
[dependencies]
pebble-engine = "1.0"
```

It's imported as `pebble`:

```rust,ignore
use pebble::app::App;
use pebble::graphics::GraphicsPlugin;

fn main() {
    App::new()
        .with_logging()
        .add_plugin(GraphicsPlugin)
        .run();
}
```

`GraphicsPlugin` opens a window, acquires a GPU backend, and registers every built-in asset type. That's enough to compile and run — an empty window that clears to black every frame. From here:

- [Apps and Plugins](./apps-and-plugins.md) and [Systems and Stages](./systems-and-stages.md) — how to add your own logic.
- [The Asset Pipeline and Handles](./the-asset-pipeline.md) — load a mesh and texture.
- [Recording a Render Pass](./rendering-pass-recording.md) — draw something.

## Running on the web

Pebble also builds for `wasm32-unknown-unknown`. See [Running on the Web](./running-on-the-web.md).
