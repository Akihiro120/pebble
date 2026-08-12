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

## `BindGroupTarget`

If you're building a custom pipeline type that a `BindingInstance<T>` should be able to bind against (mirroring how `GPUMaterial`/`GPUCompute` work), implement `BindGroupTarget` for your uploaded type:

```rust,ignore
impl BindGroupTarget for GPUMyPipeline {
    fn bind_group_layout(&self) -> &BindGroupLayout { &self.layout }
    fn binding_entries(&self) -> &[BindingEntry] { &self.entries }
}
```

Then `BindingInstance<MyPipeline>` works exactly like `MaterialInstance`/`ComputeInstance` — see [Materials](./materials.md).

## Raw buffers, textures, and bind groups

For anything that doesn't need the asset system's retry/dependency machinery at all — a one-off buffer, a render target texture — build them directly with `BufferBuilder`, `RenderTargetTextureBuilder`, and `BindGroupBuilder`. See [Buffers](./buffers.md), [Textures](./textures.md), and [Bind Groups and Layouts](./bind-groups.md).
