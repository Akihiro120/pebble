# The Asset Pipeline and Handles

How a CPU-side description (mesh data, a texture descriptor) becomes a GPU-side object (a vertex buffer, an uploaded texture) on its own schedule, without you writing upload/retry logic by hand.

## `AssetSource` and `Asset<B>`: describing a conversion

`B` is the backend type — for the rest of this book, `pebble::wgpu::backend::WGPUBackend`. Two traits work together: `AssetSource` declares the GPU-side output type, and `Asset<B>` implements the conversion — both are implemented on the **CPU-side** source type, not the GPU result:

```rust
use pebble::assets::upload::{Asset, AssetSource};

struct Mesh { vertices: Vec<Vertex>, indices: Vec<u32> }
struct GPUMesh { vertex_buffer: Buffer, index_buffer: Buffer, index_count: u32 }

impl AssetSource for Mesh {
    type Processed = GPUMesh;  // the GPU-side result type
}

impl Asset<WGPUBackend> for Mesh {
    type Deps<'a> = ();   // no extra dependencies

    fn upload<'a>(&self, backend: &WGPUBackend, _deps: &()) -> Option<GPUMesh> {
        // create GPU buffers from self's data
        Some(GPUMesh { /* ... */ })
    }
}
```

`upload` returning `None` means "not ready yet, retry next tick." `Deps` names extra resources `upload` needs beyond the backend itself (a shared bind group layout, a camera — see [Custom GPU Resources](./custom-gpu-resources.md) for a real one); if a `Deps` resource isn't present yet, the whole upload is skipped and retried, same as a missing `Res<T>` on an ordinary system.

## `Assets<T>`: unified CPU+GPU storage

`Assets<T>` stores both the CPU source (`T`) and its GPU-side result (`T::Processed`) per entry — one resource, both sides. `.get(handle)` returns `Option<&T::Processed>` — the uploaded GPU object, or `None` if it hasn't finished uploading yet:

```rust
fn render(meshes: Res<Assets<Mesh>>, mut query: Query<&Handle<Mesh>>) {
    for handle in query.iter() {
        let Some(gpu_mesh) = meshes.get(*handle) else { continue }; // Option<&GPUMesh>
        // gpu_mesh.vertex_buffer, gpu_mesh.index_count, ...
    }
}
```

There is no separate `ProcessedAssets<GPUType>` resource — `Res<Assets<Mesh>>` gives you both sides. Insert CPU data with `assets.insert(name, value)` or via a builder's `.build_asset(name, &mut assets)`.

## `AssetPlugin`: wiring it up

`AssetPlugin::<B, T>::new()` — where `T` is the **CPU source type** — registers the upload pipeline:

- `Assets<T>` — the unified store (source + processed), inserted automatically.
- A sync system that drains the dirty queue every tick, calling `T::upload` for each pending entry, re-queuing anything that returned `None`.

No manual ordering, no callbacks — insert source data, and the processed value shows up in `Assets<T>` whenever `upload` first succeeds.

`WGPUPlugin` already registers `AssetPlugin` for every built-in type (`Mesh`, `Texture`, `Material`, etc.) — you only need to call it yourself for custom asset types you define.

## `Handle<T>`: a typed reference

`Assets<T>::insert(name, value)` (or, from a builder, `.build_asset(name, &mut assets)`, which calls it for you) returns a `Handle<T>` — a small, `Copy`, typed key into that store:

```rust
let quad: Handle<Mesh> = MeshBuilder::new(vertices, indices).build_asset("quad", &mut meshes);
```

A `Handle<T>` doesn't keep anything alive on its own; it's just a lookup key, cheap to store on a component or clone around. `Handle::default()` is the null handle — the same sentinel every lookup already treats as "not present."

Internally, `Handle<T>` wraps an untyped `RawAssetHandle`. You'll see `RawAssetHandle` directly when code needs to cross from one `Assets<T>` into another — for example, a material instance stores its parent material as a raw handle, and the render system reconstructs the typed handle for lookup:

```rust
// instance.target is a RawAssetHandle — reconstruct a typed Handle for lookup:
let material = materials.get(Handle::<Material>::new(instance.target))?;
```

## One-off GPU resources: startup systems

Some things aren't authored data at all — there's exactly one of them in the whole app, and they just need a backend to exist before they can be constructed. A depth texture is the canonical example: not loaded from a file, not one of many, but genuinely can't exist before the GPU device does.

Use an ordinary startup system that inserts the resource via `Commands::insert_resource`. Returning `Option<()>` makes it a once-system (see [Systems and Stages](./systems-and-stages.md#run-once)) that retries if `WGPUBackend` isn't ready yet:

```rust
fn init_depth_texture(mut commands: Commands, backend: Res<WGPUBackend>) -> Option<()> {
    let view = RenderTargetTextureBuilder::new(
        backend.surface_width(),
        backend.surface_height(),
        TextureFormat::Depth16Unorm,
    )
    .with_label("depth")
    .with_usage(TextureUsages::RENDER_ATTACHMENT)
    .build(&backend);
    commands.insert_resource(DepthTexture { view });
    Some(())
}
```

```rust
.add_system(SystemStage::Startup, init_depth_texture)
```

Everywhere else in the app, `Res<DepthTexture>` just looks like any other resource. If you need *more than one* instance of something (multiple textures, multiple materials), that's `Asset<B>` + `Handle<T>` from earlier on this page, not a startup system — the dividing line is exactly "one of, ever" vs. "a pool of, addressed by handle."

## Why two `AssetSync` stages?

Some assets depend on *other* assets — a material instance needs its material to already be uploaded, a material might need a camera bind group layout that's itself inserted by a startup system. `AssetSync` runs plain assets (mesh, texture — no cross-asset dependency); `AssetSyncDeps` runs anything depending on another asset. Both are re-run to convergence every tick (see [Systems and Stages](./systems-and-stages.md#stages-when-a-system-runs)), so a multi-level dependency chain resolves itself over however many ticks it takes, with each level just declaring what it needs via `Deps` and trusting the framework to sequence it correctly.
