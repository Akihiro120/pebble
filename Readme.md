# Pebble

[![Examples](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml)
[![Check](https://github.com/Akihiro120/pebble/actions/workflows/check.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/check.yml)
[![Crates.io](https://img.shields.io/crates/v/pebble-engine.svg)](https://crates.io/crates/pebble-engine)
[![docs.rs](https://img.shields.io/docsrs/pebble)](https://docs.rs/pebble-engine)
[![License](https://img.shields.io/crates/l/pebble.svg)](#license)

A modular ECS framework for building render engines in Rust. Pebble provides the application loop, plugin system, resource management, and a GPU asset pipeline — but makes **no rendering decisions for you**. Batching, depth, post-processing, shaders, and draw calls are all yours to own.

New to Pebble? **[Learn Pebble](https://akihiro120.github.io/pebble/)** is a how-to guide, organized by feature — one page per topic ("how do I add a system," "how do I render into a texture") rather than a tutorial you read start to finish. This Readme is the dense single-page version of the same material.

> [!WARNING]
> Pebble is built primarily for my own projects. It is shared publicly and you are free to use it, but expect breaking changes without notice. My own use cases drive priorities over external feature requests.

---

## Design philosophy

Most graphics frameworks force you into their renderer. Pebble does the opposite: it gives you the plumbing and gets out of the way.

- **Bring your own graphics API.** Implement the `Backend` + `FrameOperations` traits for wgpu, Metal, Vulkan, or anything else.
- **Bring your own windowing.** Implement `WindowProvider` + `WindowRunner` for winit, SDL2, or a headless context.
- **Bring your own assets.** Implement `Asset<B>` to describe how a CPU-side value becomes a GPU-side value. Pebble handles the dirty queue, retry logic, and dependency ordering automatically.
- **Compose with plugins.** Everything — windowing, the backend, asset types, game logic — is a `Plugin`. Your engine is just a list of plugins wired to an `App`.

---

## Core concepts

### App and plugins

```rust
App::new()
    .add_plugin(MyWindowPlugin)
    .add_plugin(MyBackendPlugin)
    .add_plugin(MyGamePlugin)
    .build()
    .run();
```

`build()` runs all plugin registrations and validates that every declared resource dependency has a provider. There's no dedicated "run once at startup" stage or step — see [`.once()`](#run-once) below for that. `run()` hands the app to the runner installed by your window plugin.

### Systems and stages

Systems are plain Rust functions. Parameters are declared in the function signature and fetched automatically:

```rust
fn my_system(
    time:   Res<Time>,           // immutable resource borrow
    mut rb: ResMut<RigidBodies>, // mutable resource borrow
    mut q:  Query<&mut Transform>, // ECS query
    mut cmd: Commands,           // deferred world mutations
) { … }
```

Systems are registered at a `SystemStage` that determines when they run each tick:

| Stage | Purpose |
|---|---|
| `PreUpdate` | Before main logic (e.g. input, time) |
| `Update` | Main game logic |
| `PostUpdate` | After main logic |
| `PreRender` | Prepare render data, poll backend |
| `AssetSync` | Upload CPU assets to the GPU backend |
| `AssetSyncDeps` | Upload assets that depend on other GPU assets |
| `Render` | Issue draw calls |
| `PostRender` | Present the frame |

`AssetSync` and `AssetSyncDeps` are prioritized: they're run to convergence (repeated until a full pass inserts no new resources) at the very front of every tick and again after every other stage, so newly queued asset/resource work is drained immediately rather than waiting for the next tick's front pass.

Systems that declare a hard requirement — a bare `Res<T>`/`ResMut<T>` parameter — are checked before their stage runs. What happens when the resource isn't there yet depends on whether anything has registered it as eventually arriving:

- If some plugin called `app.provides::<T>()` (`GraphicsPlugin` does this for the GPU backend; `LazyResourcePlugin` does it for the type it constructs), the system just waits quietly and is retried next tick.
- Otherwise, `App` panics immediately, naming both the resource and the offending system, and suggesting `app.provides::<T>()` as the fix if the timing really is expected. This catches the common case — forgetting `app.add_resource(...)` — without also catching every legitimate "this arrives asynchronously" resource.

Wrapping a system in `.run_if::<ResourceExists<T>>()` (or any other condition — see [Run conditions](#run-conditions)) fully exempts it from this check — the condition is trusted to gate correctly.

### Run once

There's no separate "Startup" stage — instead, [`.once()`](#) turns "have I already done this" into the system's own return value, on whichever stage you register it:

```rust
fn spawn_scene(mut commands: Commands, pbr: Option<Res<PBR>>) -> Option<()> {
    let pbr = pbr?; // not ready yet — try again next tick
    if pbr.cubemap_material_inst == RawAssetHandle::default() {
        return None; // PBR exists but this field isn't populated yet — keep waiting
    }
    commands.spawn(/* ... */);
    Some(()) // done — never runs again
}

app.add_system(SystemStage::PreUpdate, spawn_scene.once());
```

Return `None` to mean "not ready, call me again next tick"; return `Some(())` to mean "done" — the system is retired permanently and never invoked again, no matter how many ticks that took. This is what replaces manually tracking a `Local<bool>` "already ran" flag, and it composes with the same hard-requirement checking as any other system: a bare `Res<T>` param is still checked (wait if provided, panic if not) before the function is even called.

### Queries

`Query<Q>` wraps an `hecs` query. Iterate it directly with `&mut query`, or use the lookup helpers when you don't need the whole result set:

```rust
fn move_system(mut q: Query<(&mut Position, &Velocity)>) {
    for (pos, vel) in &mut q {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}
```

- `query.get(entity, |item| ...)` — fetch components for one known `Entity` without scanning the rest of the query.
- `query.single()` / `query.get_single()` — expect exactly one match (the player, the active camera). `single` panics if that's not true; `get_single` returns `None` instead.

Include `Entity` in `Q` (e.g. `Query<(Entity, &Health)>`) if you need the entity id back out alongside its components.

### Run conditions

`.run_if::<C>()` gates a system (or a whole tuple passed to `add_systems`) behind a `RunCondition`, so its `SystemParam`s are only fetched — and its body only runs — when the condition holds. It's re-checked every tick:

```rust
app.add_systems(
    SystemStage::Update,
    report_expensive_setup.run_if::<ResourceExists<ExpensiveSetup>>(),
);
```

Built-in conditions: `ResourceExists<T>`, plus `And<A, B>` / `Or<A, B>` for combining conditions. Implement `RunCondition` yourself for anything else.

### Resources

Resources are singleton values stored in the ECS world. Any `hecs::Component` type can be a resource:

```rust
app.add_resource(MyConfig { … });

// In a system:
fn my_system(config: Res<MyConfig>) { … }
```

`Option<Res<T>>` is used when a resource may not exist yet — the system receives `None` and can skip its work gracefully. This is the standard way to wait for things like the GPU backend, which arrives asynchronously after startup.

### Events

`Events<T>` is a double-buffered queue: an event sent during tick `N` stays visible to every reader for the rest of `N` and all of `N + 1`, then is dropped — so a reader running anywhere in either tick sees it exactly once, regardless of whether it runs before or after the writer that sent it.

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

Each `EventReader<T>` keeps its own private read cursor (like `Local<T>`), so multiple independent readers of the same event type don't interfere with each other.

### Async systems & background tasks

`BackgroundTasksPlugin::new(worker_count)` registers a small worker-thread pool (`Res<BackgroundTasks>`) for offloading work off the main thread. Three ways to use it, depending on what you need back:

| I want... | Use | Result delivery |
|---|---|---|
| A blocking closure run off-thread, native only | `BackgroundTasks::spawn_blocking` | poll the returned `TaskHandle<T>` yourself |
| A future (`async`/`.await`) run off-thread, web-compatible | `BackgroundTasks::spawn_async` | poll the returned `TaskHandle<T>` yourself |
| A whole system that's fire-and-forget async, no result needed | `.detach()` | nothing — genuinely fire-and-forget |
| A future whose result should show up as an ordinary event | `AsyncEventWriter<T>` | automatic — arrives on `EventReader<T>` |

`spawn_blocking` is the odd one out: it doesn't work on web (there are no OS threads to block on in a browser tab), which is why it's named after what makes it platform-specific rather than being the plain `spawn`.

```rust
fn load_level(tasks: Res<BackgroundTasks>) {
    let mut handle = tasks.spawn_blocking(|| std::fs::read("level.bin"));
    // poll `handle.try_recv()` from a system on a later tick
}
```

For actual `async`/`.await` work (not just a blocking closure), `BackgroundTasks::spawn_async` drives a future to completion the same way — on a worker thread natively, on the browser's microtask queue on web, so it doesn't need to be `Send` there.

A task that panics doesn't just vanish: on native, `spawn_blocking`/`spawn_async` catch the panic, log it via `tracing::error!` with the message, and the worker thread keeps running (one bad task no longer permanently costs the pool a thread). `TaskHandle::try_recv()` still returns `None` for a panicked task — same as "still pending," for callers that don't care why — but `TaskHandle::poll()` returns `TaskStatus::Panicked(message)` instead, so code that needs to tell the two apart can:

```rust
match handle.poll() {
    TaskStatus::Pending => {}
    TaskStatus::Ready(bytes) => { /* ... */ }
    TaskStatus::Panicked(message) => tracing::error!("load_level task failed: {message}"),
}
```

A whole *system* can also be fire-and-forget async via `.detach()`: the system runs synchronously as usual (fetching its `SystemParam`s), but instead of doing work directly it returns a future, which the scheduler hands to `BackgroundTasks::spawn_async` and moves on from immediately:

```rust
fn save_screenshot(tasks: Res<BackgroundTasks>) -> impl Future<Output = ()> + Send + 'static {
    let tasks = tasks.clone();
    async move {
        // ... write to disk ...
    }
}

app.add_system(SystemStage::Update, save_screenshot.detach());
```

A real `async fn` can't be used directly as a system — its returned future borrows every parameter, so it's never `'static` on its own. Extract the owned pieces you need in the ordinary (synchronous) function body, then move only those into the `async move` block you return.

`.detach()` is genuinely fire-and-forget: nothing delivers the result back automatically. When you need the result, `AsyncEventWriter<T>` is the friendlier alternative — it combines `BackgroundTasks::spawn_async` with the event system so a background task's result arrives as a normal `T` event once it resolves, no manual polling required. It sits next to `EventWriter<T>` in the same reader/writer vocabulary — `EventWriter::send` enqueues an event now, `AsyncEventWriter::spawn` enqueues one once the future resolves:

```rust
app.add_async_event::<ReadbackDone>();

fn start_readback(events: AsyncEventWriter<ReadbackDone>, buffer: Res<SomeGpuBuffer>) {
    let future = buffer.0.read(); // buffer.0: pebble::wgpu::buffer::Buffer
    events.spawn(async move { ReadbackDone(future.await) });
}

fn on_readback(mut reader: EventReader<ReadbackDone>) {
    for event in reader.iter() {
        // event.0 is the Vec<u8> read back from the GPU
    }
}
```

`Buffer::read`/`read_as::<T>` (in the built-in wgpu module — see [Using the built-in wgpu module](#using-the-built-in-wgpu-module)) return exactly this kind of future: a GPU→CPU buffer copy, driven off the main thread and delivered as an event once the GPU finishes mapping it.

### The asset pipeline

The `Asset<B>` trait describes how a CPU-side source type is converted to a processed type using backend `B`:

```rust
impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;       // stored in Assets<Mesh>
    type Deps<'a> = ();       // no extra dependencies

    fn upload<'a>(source: &Mesh, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        // create GPU buffers from source data
        Some(GPUMesh { … })
    }
}
```

`B` is generic — it need not be a GPU backend. Use `B = ()` for CPU-to-CPU transforms (decompression, format conversion), or any other service type for audio, networking, etc.

Registering `AssetPlugin::<B, T>::new()` wires up the full pipeline automatically:
- `Assets<T::Source>` — stores raw CPU data, tracks a dirty queue.
- `ProcessedAssets<T>` — stores the converted results, indexed by the same handles.
- A sync system on `AssetSync` that drains the dirty queue each tick, calling `T::upload` for each pending entry.

If `upload` returns `None` the handle is re-queued for the next tick. If a `Deps` resource is missing the whole sync system waits until it appears. No manual ordering or callbacks needed.

A handle that keeps requeuing forever — a permanently missing `Deps` resource, `upload` that always returns `None` — doesn't stay invisible: after 300 ticks stuck, the pipeline escalates from a quiet `debug!` to a `warn!` naming the asset and how long it's been retrying, repeating every 300 ticks after that rather than either going silent again or spamming every tick. [`LazyResource`](#lazy-resources) construction gets the same treatment.

### Lazy resources

`LazyResource<B>` complements `Asset<B>` for resources that have **exactly one instance** in the whole app and need a device to be constructed, but don't come from authored data and don't need a `Handle` or a pool entry.

```rust
impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let texture = backend.device.create_texture(/* Depth16Unorm … */);
        let view    = texture.create_view(&Default::default());
        Some(DepthTexture { texture, view })
    }
}
```

Register with `LazyResourcePlugin`. The plugin adds a system to `AssetSyncDeps` that waits for `B` and all `Deps` to be present as resources, calls `construct` once, inserts the result via `Res<T>`, and never runs again. If `construct` returns `None` it retries next tick.

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, DepthTexture>::new())
```

Good candidates: depth textures, camera uniform buffers, shared bind group layouts — anything that is one-of-a-kind and needs a backend before it can exist. If you need more than one instance of something, use `Asset<B>` + `Handle` instead.

---

## Quick start

Add to `Cargo.toml`:

```toml
[dependencies]
pebble-engine = "0.17"
```

The minimal application — clear the screen to a colour:

```rust
use pebble::prelude::*;

fn main() {
    App::new()
        .add_plugin(WindowPlugin::<MyWindow>::new(WindowConfig {
            title: "Hello Pebble".to_string(),
            width: 800,
            height: 600,
        }))
        .add_plugin(GraphicsPlugin::<MyBackend, MyWindow>::new())
        .add_plugin(RenderPlugin::<MyBackend>::new())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<MyBackend>>) {
    if let Some(mut active) = frame.active() {
        let mut _pass = active.render_context([0.1, 0.1, 0.1, 1.0]);
        // draw calls go here
    }
}
```

`MyWindow` implements `WindowProvider + WindowRunner`. `MyBackend` implements `Backend + FrameOperations`. See the examples below for complete, runnable implementations using wgpu and winit.

---

## Examples

The examples are standalone crates that share a `examples/common` crate providing a wgpu + winit backend implementation, except `ecs_basics`, which needs no window or graphics backend at all. They are ordered by complexity and each has a step-by-step README, except `ecs_basics`, which is documented inline.

| Example | Description |
|---|---|
| [ecs_basics](examples/ecs_basics/src/main.rs) | No window/backend: components, resources, queries, commands, `Local<T>`, `LazyResource`, and `run_if` |
| [clear_screen](examples/clear_screen/README.md) | Minimal app: open a window and clear it each frame |
| [hello_triangle](examples/hello_triangle/README.md) | Draw a triangle using the asset pipeline |
| [textured_quad](examples/textured_quad/README.md) | Texture mapping and asset-to-asset dependencies |
| [orbit_camera](examples/orbit_camera/README.md) | 3D camera, depth buffer, lazy resources, and the full plugin system |
| [wgpu_showcase](examples/wgpu_showcase/src/main.rs) | The same kind of scene as `textured_quad`, built with the built-in `pebble::wgpu` module instead of a hand-rolled `Backend`/`Asset` — see [Using the built-in wgpu module](#using-the-built-in-wgpu-module) |

Run any example from its directory:

```sh
cd examples/hello_triangle
cargo run
```

> Compiled shaders (SPIR-V) are pre-built in `examples/assets/shaders/compiled/`. If you modify the GLSL sources, recompile them with `python3 examples/compile_shaders.py`.

---

## Using the built-in wgpu module

`pebble::wgpu` is a ready-made `Backend`/`FrameOperations` implementation on top of `wgpu`, plus a higher-level, descriptor-based layer for materials, meshes, and textures — a much shorter path than implementing `Asset`/`Backend` by hand (see [Implementing a backend](#implementing-a-backend) below for that path). One plugin replaces `WindowPlugin` + `GraphicsPlugin` + `RenderPlugin` + one `AssetPlugin` per asset type:

```rust
use pebble::prelude::*;
use pebble::wgpu::{
    backend::WGPUPlugin,
    material::MaterialDescriptor,
    mesh::MeshDescriptor,
    textures::TextureDescriptor,
};

App::new()
    .add_plugin(WGPUPlugin::new(WindowConfig {
        title: "My Game".to_string(),
        width: 1280,
        height: 720,
    }))
    .add_system(SystemStage::PreUpdate, setup.once())
    .add_system(SystemStage::Render, render)
    .build()
    .run();
```

`WGPUPlugin` registers the mesh, material, material-instance, texture, texture-array, cubemap, compute, and sampler asset pipelines all at once — describe what you want with a `*Descriptor` (`MeshDescriptor`, `MaterialDescriptor`, `TextureDescriptor::from_file(...)`, ...) and insert it into the matching `Assets<T>`, the same way you would with a hand-rolled `Asset` type. See the [wgpu_showcase](examples/wgpu_showcase/src/main.rs) example for a complete scene, and `Buffer::read`/`read_as::<T>` (covered in [Async systems & background tasks](#async-systems--background-tasks)) for GPU→CPU readback.

### Profiler overlay (optional)

Enable the `profiler` Cargo feature for an opt-in CPU frame-timing/telemetry plugin with an `egui`-rendered overlay:

```toml
pebble-engine = { version = "0.17", features = ["profiler"] }
```

```rust
use pebble::wgpu::profiler::{Profiler, ProfilerPlugin};

App::new()
    .add_plugin(WGPUPlugin::new(config))
    .add_plugin(ProfilerPlugin) // anywhere after WGPUPlugin
    // ...
    .build()
    .run();
```

`Res<Profiler>` gives you `fps()`/`frame_time()` anywhere, plus custom timed sections from any system:

```rust
fn physics_step(profiler: Res<Profiler>, /* ... */) {
    let _span = profiler.section("physics"); // recorded when this drops
    // ... do physics work ...
}
```

The overlay draws in its own pass on top of whatever your own render systems already drew — no changes needed to your existing rendering code. Works on native and web (via a small, sound `unsafe impl Send/Sync` specific to `wasm32`'s single-threaded execution model — see the module docs for why). CPU-side timing only for now; GPU timestamp-query sections are a planned follow-up, not in this version.

Off by default — `egui`/`egui-wgpu` aren't pulled into your build unless you enable the feature.

---

## Implementing a backend

To use Pebble with your own graphics API, implement two traits:

**`FrameOperations`** — represents one acquired frame:

```rust
impl FrameOperations for MyFrame {
    type Context<'a> = MyRenderPass<'a>;  // what you draw with
    type Attachment      = MyTextureView;
    type DepthAttachment = MyTextureView;

    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_> { … }
}
```

**`Backend`** — manages the swapchain and device:

```rust
impl Backend for MyBackend {
    type Frame = MyFrame;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>) {
        // create device/swapchain synchronously or on a thread, then:
        sender.send(MyBackend { … });
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError> { … }
    fn present(&mut self, frame: Self::Frame) { … }
}
```

`init` always delivers the backend through an `InitSender`, whether you do it synchronously (call `sender.send` before returning) or asynchronously (spawn a thread/task and call `sender.send` when ready). The framework polls the channel each `PreRender` tick until the backend arrives.

`AcquireError::Transient` (swapchain out of date, and similar) just skips the frame and retries next tick — normal, expected, no action needed. `AcquireError::Fatal` means `acquire` itself judged the failure unrecoverable (a lost device, a destroyed surface); `RenderPlugin` logs it once, stops calling `acquire` again, and inserts a `RenderFailure` resource instead of silently retrying a dead backend forever. Nothing panics or exits on your behalf — only your application knows whether the right response is an error screen, a full backend re-init, or something else — so react to it explicitly wherever that decision belongs:

```rust
fn on_render_death(failure: Res<RenderFailure>) -> Option<()> {
    eprintln!("rendering has permanently stopped: {}", failure.message);
    Some(()) // .once() — react exactly once, not every tick after
}

app.add_system(SystemStage::PostRender, on_render_death.once());
```

---

## Web/wasm

Pebble targets `wasm32-unknown-unknown` as well as native — `cargo build --target wasm32-unknown-unknown` builds the library, and the built-in `pebble::wgpu` backend already branches internally where the two platforms need different handling (GPU backend selection, buffer-mapping/readback driven by the browser's microtask queue instead of a worker thread, and so on).

To run in a browser:

- Add a `<canvas id="wgpu_canvas"></canvas>` to your `index.html` — `pebble::wgpu::window::WinitWindow` looks for that element by id and renders into it.
- Pulling in `web-sys`/`wasm-bindgen`/`wasm-bindgen-futures` (already wasm32-only dependencies of this crate) and bundling with `wasm-bindgen`/`trunk`/`wasm-pack` is up to your own build setup — Pebble doesn't prescribe one.
- `BackgroundTasksPlugin`'s worker-thread pool has no OS threads to spawn on web, so `BackgroundTasks::spawn_blocking` (a blocking closure) queues jobs that never run there. `BackgroundTasks::spawn_async` (and everything built on it — `.detach()`, `AsyncEventWriter<T>`, `Buffer::read`) *is* web-compatible: it drives the future through the browser's microtask queue instead of a worker thread.
- `pebble::wgpu::window::WinitWindow` installs a [`console_error_panic_hook`](https://crates.io/crates/console_error_panic_hook) automatically, so a panic shows up as a real message and Rust-side stack trace in the browser's console instead of an opaque trap — no setup needed if you're using it. Building your own `WindowProvider` for web instead means installing one yourself.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
