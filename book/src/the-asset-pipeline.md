# The Asset Pipeline and Handles

How a CPU-side description (mesh data, a texture descriptor) becomes a GPU-side object (a vertex buffer, an uploaded texture) on its own schedule, without you writing upload/retry logic by hand.

## `Asset<B>`: describing a conversion

`B` is the backend type — for the rest of this book, `pebble::wgpu::backend::WGPUBackend`. `Asset<B>` describes one conversion: a `Source` type (what you author) becomes `Self` (what gets used at render time):

```rust
impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;   // stored in Assets<Mesh>
    type Deps<'a> = ();   // no extra dependencies

    fn upload<'a>(source: &Mesh, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        // create GPU buffers from source data
        Some(GPUMesh { /* ... */ })
    }
}
```

`upload` returning `None` means "not ready yet, retry next tick" — the same convention as `.once()` (see [Systems and Stages](./systems-and-stages.md#run-once)), but per-asset instead of per-system. `Deps` names extra resources `upload` needs beyond the backend itself (a shared bind group layout, a camera — see [Custom GPU Resources](./custom-gpu-resources.md) for a real one); if a `Deps` resource isn't present yet, the whole upload is skipped and retried, same as a missing `Res<T>` on an ordinary system.

`B` doesn't have to be a graphics backend at all — `B = ()` works for a pure CPU-to-CPU transform (decompression, format conversion), and any other service type works for audio, networking, or whatever else fits the same "raw data in, processed value out, maybe needs something else to exist first" shape.

## `AssetPlugin`: wiring it up

You rarely implement `upload` by hand for the built-in `wgpu` types (`WGPUPlugin` already does it) — but registering `AssetPlugin::<B, T>::new()` is what turns an `Asset<B>` impl into a working pipeline:

- `Assets<T::Source>` — stores raw CPU data, tracks which entries are dirty.
- `ProcessedAssets<T>` — stores the converted (GPU-side) results, indexed by the same handles as the source.
- A sync system on `AssetSync` that drains the dirty queue every tick, calling `T::upload` for each pending entry, re-queuing anything that returned `None`.

No manual ordering, no callbacks — insert source data, and the processed value shows up in `ProcessedAssets<T>` whenever `upload` first succeeds.

## `Handle<T>`: a typed reference

`Assets<T>::insert(name, value)` (or, from a builder, `.build_asset(name, &mut assets)`, which calls it for you) returns a `Handle<T>` — a small, `Copy`, typed key into that store:

```rust
let quad: Handle<Mesh> = MeshBuilder::new(vertices, indices).build_asset("quad", &mut meshes);
```

A `Handle<T>` doesn't keep anything alive on its own; it's just a lookup key, cheap to store on a component or clone around. `Handle::default()` is the null handle — the same sentinel every lookup already treats as "not present," useful as a placeholder before an asset exists yet.

Internally, `Handle<T>` wraps an untyped `RawAssetHandle` — you'll see `RawAssetHandle` directly (via a handle's `.id` field) whenever code needs to cross between a *source* type's `Assets<T>` and a *differently-typed* `ProcessedAssets<U>`, since a single `Handle<T>` can't type-correctly refer to both sides of that conversion at once. [Materials](./materials.md#a-material-instance-concrete-resources-bound-to-a-material) shows exactly where this comes up.

## `LazyResource<B>`: exactly one, constructed on demand

Some things aren't authored data at all — there's exactly one of them in the whole app, and they just need a backend to exist before they can be constructed. A depth texture is the canonical example: not loaded from a file, not one of many, but genuinely can't exist before the GPU device does.

```rust
impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let view = RenderTargetTextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
            .usage(TextureUsages::RENDER_ATTACHMENT)
            .build(backend);
        Some(DepthTexture { view })
    }
}
```

Register with `LazyResourcePlugin::<WGPUBackend, DepthTexture>::new()`. It adds a system to `AssetSyncDeps` that waits for the backend (and any `Deps`) to exist, calls `construct` exactly once, inserts the result as an ordinary `Res<DepthTexture>`, and never runs again. Everywhere else in the app, a `DepthTexture` just looks like any other resource — the "wait for it to become constructible" logic lives entirely in this one plugin, not scattered across every system that needs it.

If you need *more than one* instance of something (multiple textures, multiple materials), that's `Asset<B>` + `Handle<T>` from earlier on this page, not `LazyResource` — the dividing line is exactly "one of, ever" vs. "a pool of, addressed by handle."

## Why two `AssetSync` stages?

Some assets depend on *other* assets — a material instance needs its material to already be uploaded, a material might need a camera bind group layout that's itself a `LazyResource`. `AssetSync` runs plain assets (mesh, texture — no cross-asset dependency); `AssetSyncDeps` runs `LazyResource`s and anything depending on another `ProcessedAssets<T>`. Both are re-run to convergence every tick (see [Systems and Stages](./systems-and-stages.md#stages-when-a-system-runs)), so a multi-level dependency chain resolves itself over however many ticks it takes, with each level just declaring what it needs via `Deps` and trusting the framework to sequence it correctly.
