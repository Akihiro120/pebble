# Example: Clear Screen

The simplest possible Pebble application. Opens a window and fills it with a solid colour every frame. No assets, no entities — just the three core plugins wired together.

---

## What you will learn

- How to create an `App` and chain plugins
- The role of `WindowPlugin`, `GraphicsPlugin`, and `RenderPlugin`
- How to write a system that accesses the current frame
- The `SystemStage` ordering and where rendering belongs

---

## Step 1 — Create the App and add the window plugin

```rust
App::new()
    .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
        title: "Clear Screen",
        width: 1920,
        height: 1080,
    }))
```

`WindowPlugin` is generic over a `WindowRunner` implementation. Here we use `WinitWindow` from `examples-common`, which wraps [winit](https://github.com/rust-windowing/winit).

When built, `WindowPlugin`:
1. Creates the OS window using `WindowConfig`.
2. Inserts a `WindowResource<WinitWindow>` so other systems can query the handle and size.
3. Replaces the default app runner with winit's event loop, which calls `app.update()` on every `RedrawRequested` event.

---

## Step 2 — Add the graphics backend plugin

```rust
.add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
```

`GraphicsPlugin` is generic over a `Backend` and a `WindowProvider`. It:
1. On `Startup`: calls `Backend::init` with the window handle and a one-shot `InitSender`. The wgpu backend uses `pollster::block_on` to drive the async device/surface creation on the current thread, then sends the finished `WGPUBackend` through the channel.
2. On `PreRender` each frame: polls the channel. Once the backend arrives it inserts it as a resource and removes the pending marker.
3. On `PreRender` each frame: forwards the current window size to `Backend::resize` so the swapchain stays in sync with the window.

Until the backend resource exists, the render system does nothing — all resource parameters typed as `Option<Res<…>>` return `None` gracefully.

---

## Step 3 — Add the render plugin

```rust
.add_plugin(RenderPlugin::<WGPUBackend>::new())
```

`RenderPlugin` adds a `CurrentFrame<WGPUBackend>` resource and two hidden systems:

| Stage | System | What it does |
|---|---|---|
| `PreRender` | `begin_frame` | Calls `backend.acquire()`. On success, stores the frame in `CurrentFrame`. On transient error (swapchain out of date), stores `None` and skips the tick. |
| `PostRender` | `end_frame` | Calls `backend.present(frame)`, submitting the command buffer and presenting the swapchain texture. |

Rendering systems run between these two, during `Render`.

---

## Step 4 — Write the render system

```rust
.add_system(SystemStage::Render, render)
```

```rust
fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let _pass = active.render_context([0.2, 0.3, 0.3, 1.0]);
    }
}
```

`frame.active()` returns `None` if no frame was acquired this tick (backend not ready, or transient error), so the `if let` guards safely. When a frame is active, `active.render_context(color)` begins a render pass that clears the surface to `color` (RGBA) and returns the render context.

Since we only want a clear, we begin the pass and immediately drop it. The pass is submitted in `end_frame`.

---

## Step 5 — Build and run

```rust
.build()
.run()
```

`build()` runs all plugin registrations, executes startup systems, and validates that every declared required resource has a provider.

`run()` hands the `App` to the runner installed by `WindowPlugin` (the winit event loop). From that point on `app.update()` is called once per frame.

---

## Full system stage order for this example

```
Startup      → GraphicsPlugin: kick off backend init
               (winit event loop begins)
PreRender    → GraphicsPlugin: poll for backend / forward resize
               RenderPlugin:   begin_frame (acquire swapchain texture)
Render       → render (clear the screen)
PostRender   → RenderPlugin:   end_frame (submit + present)
```
