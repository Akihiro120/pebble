# Getting Started

## Adding the dependency

```toml
[dependencies]
pebble-engine = "0.20"
```

The crate is named `pebble-engine` on crates.io, but the library itself is `pebble` — everything in this book is `use pebble::...`.

## The prelude

Almost every type used in this book — `App`, `Res`/`ResMut`, `Query`, `Commands`, `Handle<T>`, `Events`/`EventReader`/`EventWriter`, `BackgroundTasks` — is re-exported from `pebble::prelude`:

```rust
use pebble::prelude::*;
```

The `wgpu` module (materials, meshes, textures, the backend itself) is deliberately *not* in the prelude — you'll import from `pebble::wgpu::{...}` explicitly starting with [Windows and Backends](./windows-and-backends.md). Keeping it separate means a project that never touches `pebble::wgpu` (a headless simulation, a server) doesn't drag `wgpu` in as a dependency's dependency in spirit, even though it's still compiled in today — the split exists so that boundary is at least visible in every import list.

## The shape of every Pebble program

Every Pebble application, no matter how small, has the same three-part shape:

```rust
fn main() {
    App::new()
        .add_plugin(/* ... */)   // 1. register capabilities
        .add_system(/* ... */)   // 2. register behavior
        .build()                 // 3. wire it all together, validate, go
        .run();
}
```

1. **Plugins** add capabilities — a window, a graphics backend, an asset type, your own game-specific setup. `App::add_plugin` just queues them; nothing runs yet.
2. **Systems** are plain functions registered against a stage (`SystemStage::Update`, `SystemStage::Render`, ...) that determines when they run each tick.
3. **`build()`** runs every queued plugin's registration logic, checks that every system's resource requirements can eventually be satisfied, and settles as much of the asset pipeline as it can synchronously. **`run()`** hands control to whichever runner the window plugin installed — normally an infinite loop calling `App::update()` once per frame.

Nothing renders yet with just this shape — that needs a window and a backend, which [Windows and Backends](./windows-and-backends.md) covers. [Apps and Plugins](./apps-and-plugins.md) is where the ECS Core pages start, using a headless example with no window at all, so the ECS vocabulary is settled before graphics enters the picture.

## Running the examples alongside this book

Pebble's repository ships six runnable examples, ordered by complexity, in `examples/` — see [Further Reading](./further-reading.md#the-examples-ordered-by-complexity) for the full list. This book leans most heavily on `ecs_basics` (no window needed) and `wgpu_showcase` (the built-in `wgpu` module) — both are good to have open in another tab as you read:

```sh
git clone https://github.com/Akihiro120/pebble
cd pebble/examples/wgpu_showcase
cargo run
```
