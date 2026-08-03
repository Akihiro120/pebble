# Where to Go From Here

## Owning the graphics backend

Everything in Part II went through `pebble::wgpu` — a ready-made `Backend`/`FrameOperations` implementation plus the descriptor-based material/mesh/texture layer on top of it. None of that is required by the framework itself. `App`, systems, resources, events, the asset pipeline — everything from Part I — work identically against a `Backend` you write by hand, for Metal, Vulkan, D3D12, or a second, differently-configured wgpu setup of your own.

Two traits, both covered conceptually in [Opening a Window](./ch07-opening-a-window.md) and [Camera, Depth, and Lazy Resources](./ch10-camera-and-depth.md) without you writing them yourself:

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

`init` always delivers the backend through an `InitSender` — synchronously (`sender.send` before returning) or asynchronously (spawn a thread, call `sender.send` once it's ready). `App` polls the channel every `PreRender` tick until the backend arrives — which is exactly the same "resource that arrives asynchronously" shape from [Chapter 2](./ch02-systems-and-resources.md#what-happens-when-a-resource-isnt-there-yet), applied to the one resource every graphical app needs most.

Once you have `Backend`/`FrameOperations`, `Asset<B>` (Chapter 6) works against your own types exactly as it does against `WGPUBackend` — write your own `Mesh`/`Material`/`Texture` types implementing `Asset<MyBackend>` in place of reaching for `pebble::wgpu`'s. `examples/hello_triangle`, `examples/textured_quad`, and `examples/orbit_camera` do exactly this against a hand-rolled wgpu backend in `examples/common` — read them once you want to see the pattern applied for real, end to end.

## The examples, ordered by complexity

| Example | What it adds |
|---|---|
| [`ecs_basics`](https://github.com/Akihiro120/pebble/tree/main/examples/ecs_basics) | Part I's material, no window |
| [`clear_screen`](https://github.com/Akihiro120/pebble/tree/main/examples/clear_screen) | A window, a hand-rolled `Backend`, nothing drawn |
| [`hello_triangle`](https://github.com/Akihiro120/pebble/tree/main/examples/hello_triangle) | A hand-rolled `Asset` pipeline, a triangle |
| [`textured_quad`](https://github.com/Akihiro120/pebble/tree/main/examples/textured_quad) | Texture loading, asset-to-asset dependencies |
| [`orbit_camera`](https://github.com/Akihiro120/pebble/tree/main/examples/orbit_camera) | Custom plugins, `LazyResource`, depth buffer, camera — the source for Chapter 10 |
| [`wgpu_showcase`](https://github.com/Akihiro120/pebble/tree/main/examples/wgpu_showcase) | Chapters 7–9's exact code, running |

## Further reading

- **API docs**: [docs.rs/pebble-engine](https://docs.rs/pebble-engine) — every type and method this book covers, plus the ones it didn't have room for (dynamic uniform/storage buffers, texture arrays, cubemaps, `Query::single`/`get_single`, ordering constraints between systems in the same stage).
- **The `Readme`**: a denser, single-page version of Part I and the `pebble::wgpu` overview — good as a quick reference once you've read this book once.
- **`learn-wgpu`**: for wgpu concepts this book takes as given — bind groups, pipelines, shader stages — [sotrh.github.io/learn-wgpu](https://sotrh.github.io/learn-wgpu/) covers them from first principles.

Pebble is under active development — expect the API to keep moving. If something in this book drifts out of date, it's a bug in the book, not a reason to distrust it wholesale: open an issue.
