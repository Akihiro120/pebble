# Opening a Window

Everything up to this point has run headless, no window at all — Part I's examples are all tested that way (`ecs_basics` sets its own runner and calls `app.update()` in a plain loop). Real graphics needs three things: a window, a GPU device, and a render loop tied to the two. `pebble::wgpu::backend::WGPUPlugin` sets up all three at once.

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

`WGPUPlugin` is a convenience bundle — under the hood it registers a window plugin, the graphics backend, the render loop, *and* every asset pipeline this book's Part II uses (mesh, material, material instance, texture, texture array, cubemap, compute, sampler). One plugin instead of one `add_plugin` call per asset type.

## Why `Option`-shaped access to the frame

`frame.active()` returns `Option<ActiveFrame<...>>`, not the frame directly. Backend initialization is asynchronous (creating a `wgpu::Device` is itself an async operation, and on native it may also be handed off to a background thread) — for the first several ticks after `build()`, there simply isn't a frame yet. `render` just does nothing on those ticks; there's no error to handle, because nothing has gone wrong. This is the same `Option<Res<T>>` pattern from Chapter 2, applied to the one resource (`CurrentFrame<B>`) that's guaranteed to start out absent in every graphical app.

`render_context(clear_color)` is a shortcut for the common case: one color attachment, cleared to `clear_color`, no depth buffer. [Camera, Depth, and Lazy Resources](./ch10-camera-and-depth.md) uses the more general `begin_pass` once a depth attachment enters the picture.

## What you should see

Run this and you get a window, cleared every frame to a dark blue-gray — nothing drawn yet, because nothing has been given to draw. That's [Your First Triangle](./ch08-first-triangle.md).
