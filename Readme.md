# Pebble

[![Examples](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml)
[![Cargo Check](https://github.com/Akihiro120/pebble/actions/workflows/check.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/check.yml)
[![Crates.io](https://img.shields.io/crates/v/pebble-engine.svg)](https://crates.io/crates/pebble-engine)
[![docs.rs](https://img.shields.io/docsrs/pebble)](https://docs.rs/pebble-engine)
[![License](https://img.shields.io/crates/l/pebble.svg)](#license)

A modular ECS framework for building render engines in Rust. Pebble provides the application loop, plugin system, resource management, and a GPU asset pipeline — but makes **no rendering decisions for you**. Batching, depth, post-processing, shaders, and draw calls are all yours to own.

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

`build()` runs all plugin registrations, executes startup systems, and validates that every declared resource dependency has a provider. `run()` hands the app to the runner installed by your window plugin.

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

Systems are registered at a `SystemStage` that determines when they run each frame:

| Stage | Purpose |
|---|---|
| `Startup` | Once at startup, before the loop |
| `AssetSync` | Upload CPU assets to the GPU backend |
| `AssetSyncDeps` | Upload assets that depend on other GPU assets |
| `PreUpdate` | Before main logic (e.g. input, time) |
| `Update` | Main game logic |
| `PostUpdate` | After main logic |
| `PreRender` | Prepare render data, poll backend |
| `Render` | Issue draw calls |
| `PostRender` | Present the frame |

### Resources

Resources are singleton values stored in the ECS world. Any `hecs::Component` type can be a resource:

```rust
app.add_resource(MyConfig { … });

// In a system:
fn my_system(config: Res<MyConfig>) { … }
```

`Option<Res<T>>` is used when a resource may not exist yet — the system receives `None` and can skip its work gracefully. This is the standard way to wait for things like the GPU backend, which arrives asynchronously after startup.

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

---

## Quick start

Add to `Cargo.toml`:

```toml
[dependencies]
pebble-engine = "0.2"
```

The minimal application — clear the screen to a colour:

```rust
use pebble::prelude::*;

fn main() {
    App::new()
        .add_plugin(WindowPlugin::<MyWindow>::new(WindowConfig {
            title: "Hello Pebble",
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
    if let Some(mut _pass) = frame.render_context([0.1, 0.1, 0.1, 1.0]) {
        // draw calls go here
    }
}
```

`MyWindow` implements `WindowProvider + WindowRunner`. `MyBackend` implements `Backend + FrameOperations`. See the examples below for complete, runnable implementations using wgpu and winit.

---

## Examples

The examples are standalone crates that share a `examples/common` crate providing a wgpu + winit backend implementation. They are ordered by complexity and each has a step-by-step README.

| Example | Description |
|---|---|
| [clear_screen](examples/clear_screen/README.md) | Minimal app: open a window and clear it each frame |
| [hello_triangle](examples/hello_triangle/README.md) | Draw a triangle using the asset pipeline |
| [textured_quad](examples/textured_quad/README.md) | Texture mapping and asset-to-asset dependencies |
| [orbit_camera](examples/orbit_camera/README.md) | 3D camera, depth buffer, time resource, and custom plugins |

Run any example from its directory:

```sh
cd examples/hello_triangle
cargo run
```

> Compiled shaders (SPIR-V) are pre-built in `examples/assets/shaders/compiled/`. If you modify the GLSL sources, recompile them with `python3 examples/compile_shaders.py`.

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

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
