# Windows and Backends

Real graphics needs three things: a window, a GPU device, and a render loop tied to the two. `pebble::wgpu::backend::WGPUPlugin` sets up all three at once.

## `WGPUPlugin`

```rust
use pebble::prelude::*;
use pebble::wgpu::backend::WGPUPlugin;

fn main() {
    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "My Game".to_string(),
            width: 1280,
            height: 720,
        }))
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let mut _pass = active.render_context([0.05, 0.05, 0.08, 1.0]);
        // draw calls go here
    }
}
```

`WGPUPlugin` is a convenience bundle — under the hood it registers a window plugin, the graphics backend, the render loop, *and* every asset pipeline the rest of this book uses (mesh, material, material instance, texture, texture array, cubemap, compute, sampler). One plugin instead of one `add_plugin` call per asset type.

## Why `Option`-shaped access to the frame

`frame.active()` returns `Option<ActiveFrame<...>>`, not the frame directly. Backend initialization is asynchronous (creating a `wgpu::Device` is itself an async operation, and on native it may also be handed off to a background thread) — for the first several ticks after `build()`, there simply isn't a frame yet. `render` just does nothing on those ticks; there's no error to handle, because nothing has gone wrong. This is the same `Option<Res<T>>` pattern from [Resources](./resources.md#a-genuinely-optional-resource--optionrest), applied to the one resource (`CurrentFrame<B>`) that's guaranteed to start out absent in every graphical app.

`render_context(clear_color)` is a shortcut for the common case: one color attachment, cleared to `clear_color`, no depth buffer. [Custom GPU Resources](./custom-gpu-resources.md) uses the more general `begin_pass` once a depth attachment enters the picture.

Run this and you get a window, cleared every frame to a dark blue-gray — nothing drawn yet, because nothing has been given to draw. [Materials](./materials.md) and [Meshes and Vertices](./meshes.md) cover that.

## Owning the graphics backend yourself

Everything else in this book goes through `pebble::wgpu` — a ready-made `Backend`/`FrameOperations` implementation plus the descriptor-based material/mesh/texture layer on top of it. None of that is required by the framework itself. `App`, systems, resources, events, the asset pipeline — everything covered in the ECS Core pages — work identically against a `Backend` you write by hand, for Metal, Vulkan, D3D12, or a second, differently-configured wgpu setup of your own.

Two traits:

**`FrameOperations`** — one acquired frame:

```rust
impl FrameOperations for MyFrame {
    type Context<'a> = MyRenderPass<'a>;  // what you draw with
    type Attachment = MyTextureView;
    type DepthAttachment = MyTextureView;

    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_> { /* ... */ }
}
```

**`Backend`** — the swapchain and device:

```rust
impl Backend for MyBackend {
    type Frame = MyFrame;

    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>) {
        // create device/swapchain synchronously or on a thread, then:
        sender.send(MyBackend { /* ... */ });
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError> { /* ... */ }
    fn present(&mut self, frame: Self::Frame) { /* ... */ }
}
```

`init` always delivers the backend through an `InitSender` — synchronously (`sender.send` before returning) or asynchronously (spawn a thread, call `sender.send` once it's ready). `App` polls the channel every `PreRender` tick until the backend arrives — the same "resource that arrives asynchronously" shape from [Resources](./resources.md#what-happens-when-a-resource-isnt-there-yet), applied to the one resource every graphical app needs most.

Once you have `Backend`/`FrameOperations`, `Asset<B>` (see [The Asset Pipeline and Handles](./the-asset-pipeline.md)) works against your own types exactly as it does against `WGPUBackend` — write your own `Mesh`/`Material`/`Texture` types implementing `Asset<MyBackend>` in place of reaching for `pebble::wgpu`'s. `examples/hello_triangle` does exactly this against a hand-rolled wgpu backend in `examples/common` — the one example in the repository that isn't built on `pebble::wgpu`, kept deliberately as the reference for this pattern — read it once you want to see it applied for real, end to end. Note that `ColorTarget`/`Pass`/`FrameOperations` are backend-agnostic by design — features specific to `pebble::wgpu::backend::WGPUBackend` (like [MSAA](./msaa.md)'s `set_msaa`) live as inherent methods on `WGPUBackend` itself, not on these shared traits, so a hand-rolled backend is never forced to support them.
