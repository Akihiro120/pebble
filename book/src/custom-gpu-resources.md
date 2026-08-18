# Custom GPU Resources

Pebble is deliberately low-level — the built-in asset types (`Mesh`, `Texture`, `Material`, `Compute`, ...) cover the common cases, but nothing stops you from working with `Buffer`/`BindGroup`/`TextureView` directly, or defining an entirely new asset type. This page is about the escape hatches.

## Your own asset type

The usual path — see [The Asset Pipeline and Handles](./the-asset-pipeline.md) for the full explanation of the `asset!` macro and why uploads retry instead of failing:

```rust,ignore
asset!(MyThing => GPUMyThing, |self, backend: &Backend| {
    Some(GPUMyThing { /* ... */ })
});

app.add_plugin(AssetPlugin::<Backend, MyThing>::new())
```

## Building a pipeline outside `Assets<T>`

`build_material`/`build_compute` — the same functions the asset upload path calls internally — are public, for callers assembling their own wiring around a `Material`/`Compute` description without going through the usual asset flow:

```rust,ignore
let (pipeline, layout) = build_material(&backend, &material_desc, &layout_pool)
    .expect("all dependencies must already be registered");
```

## A custom pipeline type of your own

There's no generic `BindGroupTarget`/`BindingInstance<T>` extension point to plug a new pipeline type into — `Material`/`Compute` each resolve their own named bind group values directly (via the internal, crate-private `params::build_bind_group`), rather than through a trait a third pipeline type could also implement. Building a genuinely new kind of pipeline (neither a render pipeline nor a compute pipeline) means following the same shape `Material`/`Compute` do rather than extending them: your own CPU-side descriptor struct, an `upload()` that compiles a `wgpu` pipeline (mirroring `build_material`/`build_compute` above) and then builds a `BindGroup` directly via `BindGroupBuilder` (see [Bind Groups and Layouts](./bind-groups.md#building-the-actual-bind-group)) against whatever entries your own type declares.

## Raw buffers, textures, and bind groups

For anything that doesn't need the asset system's retry/dependency machinery at all — a one-off buffer, a render target texture — build them directly with `BufferBuilder` and `BindGroupBuilder`. A standalone render target is just an ordinary `Texture::empty(...)` (`.with_sample_count(...)`/`.with_extra_usage(...)` for MSAA or extra usage flags) — see [Textures](./textures.md), [Buffers](./buffers.md), and [Bind Groups and Layouts](./bind-groups.md).
