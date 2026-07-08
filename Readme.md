# Pebble
[![Examples](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/examples.yml)
[![Cargo Check](https://github.com/Akihiro120/pebble/actions/workflows/check.yml/badge.svg)](https://github.com/Akihiro120/pebble/actions/workflows/check.yml)


Tools for building a render engine. Pebble handles windowing, graphics backend abstraction, and GPU asset uploads. This library is window and graphics agnostic. It doesn't make any rendering decisions for you like: batching, compute, depth, post-processing, and anything you draw is yours to build on top.

> [!WARNING]
> Pebble is built primarily for my own projects. It's shared publicly and
> you're free to use it, but expect breaking changes without notice, and
> my own use cases will drive priorities over external feature requests.

## Status
Early, actively developed. APIs are not stable. Validated against wgpu + winit on native and web (WebGPU/WebGL). Other backends are theoretically supported by the trait design but not yet implemented.

## Quick Start
``` rust
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

fn render(
    mut frame: ResMut<CurrentFrame<MyBackend>>
) {
    let Some(mut frame) = frame.get_render_context() {
        // insert render code here
    }
}
```

`MyWindow` and `MyBackend` are your own types implementing Pebble's `WindowProvider`/`PresentableWindow`/`WindowRunner` and `Backend`/`FrameOperations` traits. This is deliberate, Pebble doesn't ship a graphics api or windowing library baked in, you choose and wire in your own.

The fastest way to get these working is to copy `examples/quad-native` (wgpu + winit), which implements all traits completely and renders a quad with a color uniform.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
