# Further Reading

## The examples, ordered by complexity

```sh
git clone https://github.com/Akihiro120/pebble
cd pebble/examples/wgpu_showcase
cargo run
```

| Example | What it adds |
|---|---|
| [`ecs_basics`](https://github.com/Akihiro120/pebble/tree/main/examples/ecs_basics) | The ECS core, no window |
| [`clear_screen`](https://github.com/Akihiro120/pebble/tree/main/examples/clear_screen) | A window, `pebble::wgpu`, nothing drawn |
| [`hello_triangle`](https://github.com/Akihiro120/pebble/tree/main/examples/hello_triangle) | The one hand-rolled-`Backend` example — a triangle via raw `wgpu` |
| [`textured_quad`](https://github.com/Akihiro120/pebble/tree/main/examples/textured_quad) | Texture loading, material instances, via `pebble::wgpu` |
| [`orbit_camera`](https://github.com/Akihiro120/pebble/tree/main/examples/orbit_camera) | Startup-system GPU resources, depth buffer, orbiting camera — the source for [Custom GPU Resources](./custom-gpu-resources.md) |
| [`wgpu_showcase`](https://github.com/Akihiro120/pebble/tree/main/examples/wgpu_showcase) | The full `pebble::wgpu` material/mesh/texture pipeline, running |
| [`compute_basics`](https://github.com/Akihiro120/pebble/tree/main/examples/compute_basics) | A compute pass that doubles a buffer and reads it back — the source for [Compute Pipelines](./compute-pipelines.md) |
| [`advanced_rendering`](https://github.com/Akihiro120/pebble/tree/main/examples/advanced_rendering) | The textured quad from `textured_quad`, rendered with [MSAA](./msaa.md) and a [render bundle](./render-bundles.md) |

Every example except `hello_triangle` uses only `pebble::wgpu` — no raw `wgpu` type anywhere, matching everything else in this book. `hello_triangle` is the deliberate exception: the reference for implementing your own `Backend` (see [Windows and Backends](./windows-and-backends.md#owning-the-graphics-backend-yourself)). `ecs_basics` and `wgpu_showcase` are the two worth having open in another tab while reading this book.

## Further reading

- **API docs**: [docs.rs/pebble-engine](https://docs.rs/pebble-engine) — every type and method this book covers, plus the ones it didn't have room for.
- **The `Readme`**: a denser, single-page version of the ECS core and the `pebble::wgpu` overview — good as a quick reference alongside this book.
- **`learn-wgpu`**: for wgpu concepts this book takes as given — bind groups, pipelines, shader stages — [sotrh.github.io/learn-wgpu](https://sotrh.github.io/learn-wgpu/) covers them from first principles.

Pebble is under active development — expect the API to keep moving. If something in this book drifts out of date, it's a bug in the book, not a reason to distrust it wholesale: open an issue.
