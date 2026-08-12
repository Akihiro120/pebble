# Further Reading

- **Full API reference**: `cargo doc --open` in the pebble repository, or the published docs on [docs.rs](https://docs.rs/pebble-engine).
- **wgpu** — pebble's GPU backend. Understanding wgpu's own concepts (bind groups, pipelines, command encoders) makes the [Rendering: Building Blocks](./buffers.md) and [Rendering: Drawing](./rendering-pass-recording.md) sections click faster, since most of pebble's rendering types mirror wgpu's own closely: <https://wgpu.rs>
- **hecs** — pebble's ECS world underneath `Query`/`Commands`: <https://docs.rs/hecs>
- **Source and issues**: <https://github.com/Akihiro120/pebble>

This book covers `release`'s current feature set. If something here doesn't match what you see in the source, the source wins — please open an issue.
