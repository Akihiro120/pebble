# Pebble

[![Crates.io](https://img.shields.io/crates/v/pebble-engine.svg?logo=rust)](https://crates.io/crates/pebble-engine)
[![docs.rs](https://img.shields.io/docsrs/pebble-engine?logo=docs.rs)](https://docs.rs/pebble-engine)
[![Check](https://github.com/Akihiro120/pebble/actions/workflows/check.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/check.yml)
[![License](https://img.shields.io/crates/l/pebble-engine.svg)](#license)

A low-level, ECS-style application/graphics framework for Rust, built directly on [`hecs`](https://docs.rs/hecs), [`wgpu`](https://docs.rs/wgpu), and [`winit`](https://docs.rs/winit). Pebble gives you the app loop, a plugin system, resource management, and a CPU→GPU asset pipeline — it deliberately makes no decisions about how you render, animate, or simulate anything. There is no built-in skeletal animation, no model loader, no collision system. Pebble's job is to make sure the low-level primitives (a mesh with any vertex layout you want, a material with any shader you write, a compute pass, GPU→CPU readback) are flexible enough that you can build those things yourself without fighting the engine.

> [!WARNING]
> Pebble is under active development and its public API is not stable. Expect breaking changes without notice.

---

## Design philosophy

- **Low-level by design, not by omission.** `Mesh` doesn't force a vertex format — it's generic over any `bytemuck::Pod` type, so a custom vertex struct (position + joint indices + weights, for example) works exactly the same way the built-in one does. `Material`/`Compute` take raw WGSL you write yourself, with an explicit vertex layout and bind-group layout — nothing about the shading model is assumed.
- **You build the game-specific systems.** Skeletal animation, model loading, collision — none of it is in the engine. It's all buildable *today* against the existing low-level API (see the `Mesh`/`Material`/`ComputeInstance` primitives below); the engine's job stops at giving you the primitives to do it with, not doing it for you.
- **Compose with plugins.** Windowing, the GPU backend, asset types, your own game logic — all of it is a `Plugin`. An `App` is built by chaining `.add_plugin(...)` calls.
- **Async work resolves like a value, not a callback.** GPU backend acquisition, buffer readback, and compute results all use the same `Promise<T>`/`PromiseState` shape — poll it each tick until it's `Ready`.

---

## Core concepts

### App and plugins

`App` is consumed and returned by each builder call — a plugin's `build` takes `App` by value and hands back a (possibly modified) `App`:

```rust
App::new()
    .add_plugin(GraphicsPlugin::new())   // windowing (winit) + GPU backend (wgpu) + the built-in asset types
    .add_plugin(TimePlugin)
    .add_system(SystemStage::Ready, setup)
    .add_system(SystemStage::Update, my_game_logic)
    .run();
```

`run()` hands control to whatever runner is installed — `GraphicsPlugin` installs one that drives the app from winit's own event loop, so you don't write your own loop.

### Systems and stages

Systems are plain functions; parameters are fetched automatically from their type:

```rust
fn my_system(
    time: Read<Time>,             // immutable resource borrow
    mut hp: Write<Health>,        // mutable resource borrow
    mut q: Query<&mut Position>,  // ECS query
    mut cmd: Commands,            // deferred world/resource mutations
) { /* ... */ }
```

Systems are registered on a `SystemStage`, run in this fixed order every tick:

| Stage | Purpose |
|---|---|
| `Startup` | Runs once, before anything else — before the GPU backend exists. Pure CPU-only bootstrapping. |
| `Ready` | Runs once, automatically, the first tick the GPU backend is ready. Where one-time setup that needs `Backend` or the asset system belongs — building meshes/materials/computes, anything that isn't meant to happen every tick. A plain `Read<Backend>` here is always safe; no `Option` guard needed. |
| `AssetSync` | Uploads CPU-side assets to the GPU, retrying automatically until their dependencies are met. |
| `PreUpdate` | Before main game logic (input polling, event aging, timers). |
| `Update` | Main game logic. |
| `PostUpdate` | After main game logic. |
| `PreRender` | Acquire the frame. |
| `Render` | Issue draw calls. |
| `PostRender` | Submit and present. |

`Startup` and `Ready` are both genuinely one-shot: registering a system there doesn't need a `Local<bool>` "have I already run" guard — the stage itself is removed from the schedule the instant it runs once.

Every stage from `Ready` onward only ever runs once the GPU backend exists — `App::update()` runs a separate `gpu_schedules` set (used internally for backend acquisition) for as long as the backend isn't ready yet, and doesn't touch the regular stages at all until it is. So a bare `Read<Backend>`/`Read<Assets<T>>` anywhere in `Ready` or later is always sound.

### Resources

Resources are singleton values, fetched by type:

```rust
app.insert_resource(MyConfig { /* ... */ });

fn my_system(config: Read<MyConfig>) { /* ... */ }
```

`Option<Read<T>>`/`Option<Write<T>>` are for a resource that might not exist yet (or might never) — the system gets `None` and can skip gracefully instead of panicking.

### Queries

`Query<Q>` wraps `hecs` queries — no `hecs::*` type appears in its public surface:

```rust
fn move_system(mut q: Query<(&mut Position, &Velocity)>) {
    for (pos, vel) in q.iter() {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}
```

`query.get(entity)` fetches one known entity directly. `query.with::<R>()`/`query.without::<R>()` narrow by component presence (chainable — each returns another `Query`). `query.single()`/`query.get_single()` expect exactly one match (panicking / `None` respectively if that's not true).

### Commands

`Commands` defers entity spawns and resource inserts/removes until the end of the current stage (`Deref`s to `hecs::CommandBuffer` for spawning, plus `insert_resource`/`remove_resource`/`trigger`):

```rust
fn spawn_enemy(mut commands: Commands) {
    commands.spawn((Position::default(), Health(100)));
}
```

### Events — polling

`Events<T>` is a double-buffered queue: an event sent during tick `N` stays visible to every reader for the rest of `N` and all of `N + 1`, then is dropped, so a reader sees it exactly once regardless of when it runs relative to the writer:

```rust
app.add_event::<Damage>();

fn deal_damage(mut writer: EventWriter<Damage>) {
    writer.send(Damage(5));
}

fn on_damage(mut reader: EventReader<Damage>) {
    for event in reader.iter() {
        // ...
    }
}
```

`add_event::<T>()` is idempotent — registering the same type twice from two different plugins is a no-op, not a double-registration bug.

### Observers — subscription

For "run this specific reaction the instant something happens" instead of polling a buffer: `add_observer` registers a callback, `Commands::trigger` dispatches to every observer registered for that type. Observers run at the end of the current stage — same tick, not deferred to some later poll:

```rust
struct WaveCleared { wave: u32 }

app.add_observer(spawn_next_wave)
   .add_observer(update_wave_banner_ui);

fn combat_system(mut commands: Commands) {
    commands.trigger(WaveCleared { wave: 3 });
}

fn spawn_next_wave(trigger: Trigger<WaveCleared>) {
    // trigger derefs straight to WaveCleared — trigger.wave
}
```

Multiple `add_observer::<E>()` calls for the same `E` all fire independently.

### Promise — one-off async results

`Promise<T>`/`Fulfiller<T>` is a oneshot result you poll each tick — used for GPU backend acquisition, `Buffer::read()`, and compute dispatch readback:

```rust
fn kick_off(buffer: &Buffer, mut pending: Local<Option<Promise<Vec<u8>>>>) {
    *pending = Some(buffer.read());
}

fn check(mut pending: Local<Option<Promise<Vec<u8>>>>) {
    if let Some(promise) = pending.as_mut() {
        if let PromiseState::Ready(bytes) = promise.poll() {
            // ...
        }
    }
}
```

Not a resource, not registered anywhere — it's a plain value you store wherever fits (a `Local`, a field on your own resource/component).

### Time

Not registered automatically — add it if you need it:

```rust
app.add_plugin(TimePlugin); // Read<Time>: delta()/delta_seconds(), elapsed()/elapsed_seconds(), fps()
```

Backed by `web_time`, so it's correct on `wasm32-unknown-unknown` too. Pebble doesn't ship audio or gamepad support — this is a graphics engine; the asset pipeline makes it straightforward to wire up your own crate for either (e.g. `rodio`, `gilrs`) as a plugin.

### Windowing and input

`GraphicsPlugin` opens a window (winit) and inserts `Window`/`Input` as resources — no raw `winit` type is ever exposed:

```rust
fn movement(input: Read<Input>, window: Read<Window>) {
    if input.key_held(KeyCode::KeyW) { /* ... */ }
    if input.mouse_pressed(MouseButton::Left) { /* ... */ }
}
```

Confirmed functional on both native and `wasm32-unknown-unknown` (verified with a real headless-Chrome screenshot, not just a compile check) — the canvas is attached to the page automatically and the event loop uses the correct non-blocking entry point on web.

### The asset pipeline

`Assets<T>` holds both a value's CPU-side source and its GPU-side processed form together (the "unified" model) — no separate CPU/GPU stores to keep in sync yourself:

```rust
impl AssetSource for MyThing {
    type Processed = GPUMyThing;
}
impl Asset<Backend> for MyThing {
    type Deps<'a> = ();
    fn upload<'a>(&self, backend: &Backend, _deps: &()) -> Option<GPUMyThing> {
        Some(GPUMyThing { /* ... */ })
    }
}
```

`AssetPlugin::<Backend, MyThing>::new()` wires the retry-until-ready upload loop into `AssetSync` automatically. If `upload` returns `None` (a dependency isn't ready yet), it's retried next tick — no manual ordering.

Writing those two trait impls by hand is enough boilerplate that there's a macro for it — expands to exactly the same code:

```rust
asset!(MyThing => GPUMyThing, |self, backend: &Backend| {
    Some(GPUMyThing { /* ... */ })
});

// with dependencies — bare types, wrapped in Read<'a, _> (or a tuple of
// them for more than one) automatically
asset!(MyThing => GPUMyThing, deps: [SomePool], |self, backend: &Backend, deps| {
    Some(GPUMyThing { /* ... */ })
});
```

Every built-in asset type (`Texture`, `Mesh`, `Material`, ...) uses the trait impls directly — the macro only exists to make a *new*, user-defined asset type cheaper to write.

CPU-side source data (a `Mesh`'s vertices/indices, a `Texture`'s pixels) stays reachable via `Assets<T>::get_source`/`get_source_by_name` — e.g. to build a collision mesh from the same vertex data used to upload the render mesh — and can be explicitly released (`release_cpu_data()`) once you're done reading it, if you don't want the CPU copy sitting in memory forever. `Assets<T>::iter()` enumerates everything currently loaded.

### GPU resource builders

Each of `Texture`/`TextureArray`/`Cubemap`/`Mesh`/`Material`/`Compute`/`MaterialInstance`/`ComputeInstance` is its own builder — no separate `XBuilder` type, chained `with_*` calls terminating in `.build_asset(name, &mut assets)`:

```rust
let material = Material::new(MY_WGSL_SHADER)
    .with_vertex_layouts(vec![MyVertex::layout()])
    .with_entries(vec![my_bind_group_layout])
    .with_targets(vec![color_target])
    .build_asset("my_material", &mut materials);
```

- `Mesh<V>` is generic over any `V: bytemuck::Pod` — the built-in `Vertex` (position/uv/normal/tangent) is just the default; a custom vertex type (joint indices/weights for skinning, say) works identically.
- `Texture`/`TextureArray`/`Cubemap` share the same construction options (`from_file`/`from_data`/`empty`), the same `with_mips()`/`with_mip_count(n)` GPU-side mip generation, and matching `get_view(..., mip_level)` accessors. An empty texture can be used as a render target (post-processing, shadow maps) — confirmed via a dedicated pure-logic test, since `RENDER_ATTACHMENT` usage is granted whenever there's no source data, independent of mip count.
- `Material`/`Compute` take raw WGSL directly — no forced shading model.
- `MaterialInstance`/`ComputeInstance` bind concrete textures/buffers/samplers into a material or compute's bind group, and can be updated at runtime (`instance.update("name", &bytes)`) — this is how you drive a live GPU buffer (joint matrices for skinning, simulation state for a compute pass) from a system each tick.
- `ComputePass` + `Backend::dispatch_compute(...)` run a compute pipeline immediately, in its own command encoder — not deferred to any render stage. Read the result back with the same `Buffer::read() -> Promise<Vec<u8>>` used everywhere else.

---

## Quick start

```toml
[dependencies]
pebble = { git = "https://github.com/Akihiro120/pebble", branch = "release" }
```

```rust
use pebble::{
    app::App,
    ecs::system::SystemStage,
    graphics::GraphicsPlugin,
    time::TimePlugin,
};

fn main() {
    App::new()
        .add_plugin(GraphicsPlugin::new())
        .add_plugin(TimePlugin)
        .run();
}
```

This opens a window and drives a real GPU render loop (acquire/submit/present) with nothing drawn yet — add systems on `Ready` to build your meshes/materials, and on `Render` to issue draw calls against them (see [Core concepts](#core-concepts) above).

---

## Web/wasm

Pebble targets `wasm32-unknown-unknown` alongside native. `cargo check --target wasm32-unknown-unknown` builds clean, and windowing + GPU rendering have been verified functional in an actual browser (not just compiling) — the canvas is attached to the page automatically (`winit`'s `with_append(true)`), and the event loop uses the non-blocking wasm entry point rather than the one that blocks forever natively.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
