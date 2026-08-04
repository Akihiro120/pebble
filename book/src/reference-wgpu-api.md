# wgpu API Reference

A task-indexed lookup, not a tutorial — "I need a buffer, what do I call?" For the concepts behind any of this (why materials and instances are separate, why bind group entries need explicit visibility, ...), see the relevant chapter in [Part II](./ch06-assets-and-handles.md); this page just gets you to working code fast. Everything below is reachable via `use pebble::wgpu::prelude::*;` unless noted otherwise.

## Buffers

### A plain, uniform, or storage buffer

[`BufferBuilder`](../src/wgpu/buffers.rs) — empty (write into it later) or pre-populated:

```rust
// Empty, written into later via `queue.write_buffer`/`update_buffer`.
let camera_buffer = BufferBuilder::new().label("camera").uniform().size(64).build(&device);

// Pre-populated.
let vertex_buffer = BufferBuilder::new()
    .label("mesh vertices")
    .usage(wgpu::BufferUsages::VERTEX)
    .data(bytemuck::cast_slice(&vertices))
    .build(&device);
```

`.uniform()`/`.storage()` are shorthand for the usual `UNIFORM | COPY_DST`/`STORAGE | COPY_DST` flag pairs; use `.usage(...)` directly for anything else (vertex/index buffers, a `MAP_READ` staging buffer).

### A dynamically-offset buffer (many elements, one buffer)

[`DynamicBufferBuilder`](../src/wgpu/buffers.rs) — sized and aligned for `count` elements of `element_size` bytes, selected later via `set_bind_group`'s dynamic offset. Returns `(Buffer, stride)` — you need the stride for both the offset and later per-element writes, so it comes back alongside the buffer instead of needing to be recomputed:

```rust
let (buffer, stride) = DynamicBufferBuilder::uniform(element_size, count).build(&device);
// ... later, per element:
update_buffer_at(&queue, &buffer, index as u64 * stride, &element_bytes);
// ... at draw time:
pass.set_bind_group(0, Some(&bind_group), &[index as u32 * stride as u32]);
```

Pair with [`BindingKind::dynamic_uniform_buffer`](#a-bind-group-layout) for the layout and [`BindGroupBuilder::dynamic_buffer`](#a-bind-group) for the bind group.

### Writing to an existing buffer

Not a builder — just an existing buffer and new bytes:

```rust
update_buffer(&queue, &buffer, &new_bytes);          // whole buffer, offset 0
update_buffer_at(&queue, &buffer, offset, &bytes);   // one dynamic-offset element
```

## Bind Groups

### A bind group layout

[`BindGroupLayoutBuilder`](../src/wgpu/binding.rs) — one [`BindingKind`](../src/wgpu/binding.rs) per entry, visibility always explicit:

```rust
let layout = BindGroupLayoutBuilder::new()
    .label("camera_layout")
    .entry("camera", 0, BindingKind::uniform_buffer(wgpu::ShaderStages::VERTEX))
    .build(&device);
```

`BindingKind` constructors, one per resource shape: `texture_2d`/`texture_2d_array`/`texture_cubemap`, `storage_texture`, `sampler`/`comparison_sampler`, `uniform_buffer`/`dynamic_uniform_buffer`, `storage_buffer_read_only`/`storage_buffer_read_write`/`dynamic_storage_buffer` — every one takes `wgpu::ShaderStages` explicitly, nothing defaulted. Building a layout by hand this way is mostly for resources outside the material/compute system (a camera — see [Chapter 10](./ch10-camera-and-depth.md)); `MaterialDescriptor`/`ComputeDescriptor` below build their own layout internally from `entries`.

`.build()` panics on a duplicate `@binding(N)` — a shader/layout mismatch fails loudly here instead of at draw time.

### A bind group

[`BindGroupBuilder`](../src/wgpu/buffers.rs) — one resource per binding, against an already-built layout:

```rust
let bind_group = BindGroupBuilder::new(&layout)
    .label("camera_bind_group")
    .buffer(&camera_buffer)      // @binding(0), call order
    .build(&device);
```

`.buffer()`/`.texture()`/`.sampler()`/`.dynamic_buffer()` assign `@binding(N)` in call order starting at 0 — the common case. If your bindings aren't contiguous from 0 (looked up by name, as material/compute instances are — see below), use the `_at` variants (`.buffer_at(2, &buf)`) instead.

## Pipeline Layouts (multiple bind groups)

[`GroupLayout`](../src/wgpu/layout.rs)/[`assemble_bind_group_layouts`](../src/wgpu/layout.rs) assemble several already-built layouts into the array a pipeline layout needs, keyed by explicit `@group(N)` rather than position — this is what `MaterialDescriptor`/`ComputeDescriptor` use internally to combine their own bind group (`own_group`) with `extra_layouts`. You rarely call it directly; the field to reach for is `extra_layouts` on a material/compute descriptor:

```rust
MaterialDescriptor {
    // ... own entries occupy own_group (default Some(0)) ...
    extra_layouts: vec![OwnedGroupLayout { group: 1, layout: camera.bind_group_layout.clone() }],
    ..Default::default()
}
```

Panics on a gap or a collision in `0..=max` across `own_group` + `extra_layouts` combined — see [Chapter 10](./ch10-camera-and-depth.md#wiring-the-camera-into-the-materials-pipeline-layout).

## Materials (render pipelines)

[`MaterialDescriptor`](../src/wgpu/material.rs) → `Assets<MaterialDescriptor>::insert` → `Handle<MaterialDescriptor>`, uploaded automatically to `ProcessedAssets<GPUMaterial>` (registered by `WGPUPlugin`, no manual plugin needed):

```rust
use pebble::wgpu::{material::MaterialDescriptor, mesh::Vertex};

let material = materials.insert("lit", MaterialDescriptor {
    label: Some("lit"),
    shader_source: SHADER,
    vertex_layouts: vec![Vertex::layout()],
    entries: material_entries(),   // Vec<BindingEntry>, see "A bind group layout" above
    own_group: Some(0),            // None if entries is empty — no bind group at all
    targets: vec![wgpu::ColorTargetState {
        format: backend.config.format,
        blend: None,
        write_mask: Default::default(),
    }],
    ..Default::default()
});
```

`..Default::default()` covers `vertex_entry`/`fragment_entry` (`"vs_main"`/`"fs_main"`), `cull_mode: Some(Face::Back)`, no depth testing, fill polygon mode, no `extra_layouts`. See [Chapter 8](./ch08-first-triangle.md) (no bind group) and [Chapter 9](./ch09-textures.md) (textured).

### A material instance (concrete resources bound to a material)

[`MaterialInstanceDescriptor`](../src/wgpu/instance.rs) — matches `(name, BindingInstanceEntry)` pairs against the material's own `entries` by name:

```rust
use pebble::wgpu::instance::{BindingInstanceEntry, MaterialInstanceDescriptor};

let instance = instances.insert("brick_instance", MaterialInstanceDescriptor::new(
    material.id, // RawAssetHandle, not the typed Handle
    vec![
        ("albedo", BindingInstanceEntry::Texture(brick.id)),
        ("albedo_sampler", BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)),
    ],
));
```

`BindingInstanceEntry` variants: `Texture`/`TextureArray`/`Cubemap`/`Sampler` (all by handle/kind, resolved against existing processed assets) and `Uniform(Vec<u8>)`/`Storage(Vec<u8>)` (a buffer the instance allocates and owns itself, updatable later via `instance.update(&queue, name, &bytes)`). Full walkthrough: [Chapter 9](./ch09-textures.md).

## Compute Pipelines

[`ComputeDescriptor`](../src/wgpu/compute.rs) mirrors `MaterialDescriptor` — same `entries`/`own_group`/`extra_layouts` shape, `build_compute` panics if any entry isn't visible to *exactly* `COMPUTE`:

```rust
use pebble::wgpu::compute::ComputeDescriptor;

let pass = computes.insert("double", ComputeDescriptor {
    label: Some("double"),
    shader_source: COMPUTE_SHADER,
    entries: vec![BindingEntry {
        name: "data",
        binding: 0,
        kind: BindingKind::storage_buffer_read_write(wgpu::ShaderStages::COMPUTE),
    }],
    ..Default::default()
});
```

Dispatching happens directly against `backend.device`/`backend.queue` — no `FrameOperations`-mediated pass, since a compute pass isn't tied to an acquired frame. See [Chapter 11](./ch11-compute.md).

### A compute instance

[`ComputeInstanceDescriptor`](../src/wgpu/instance.rs) — same type as `MaterialInstanceDescriptor` (`GPUBindingInstance<T>` generic over the target), just `T = GPUCompute`:

```rust
use pebble::wgpu::instance::{BindingInstanceEntry, ComputeInstanceDescriptor};

let instance = instances.insert("double_instance", ComputeInstanceDescriptor::new(
    pass.id,
    vec![("data", BindingInstanceEntry::Storage(bytes))],
));
```

## Meshes

[`MeshDescriptor`](../src/wgpu/mesh.rs) — a fixed [`Vertex`](../src/wgpu/mesh.rs) layout (position, UV, normal, tangent) plus indices:

```rust
use pebble::wgpu::mesh::{MeshDescriptor, Vertex};

let mesh = meshes.insert("triangle", MeshDescriptor {
    vertices: vec![
        Vertex::new(glam::Vec3::new(0.0, 0.6, 0.0), glam::Vec2::ZERO, glam::Vec3::Z, glam::Vec4::new(1.0, 0.0, 0.0, 1.0)),
        // ...
    ],
    indices: vec![0, 1, 2],
});
```

Uploads to `ProcessedAssets<GPUMesh>` (`vertex_buffer`/`index_buffer`/`index_count`) automatically. See [Chapter 8](./ch08-first-triangle.md).

## Textures

Three descriptors, same `from_*` constructor pattern — decode/upload happens on the asset pipeline like any other asset, no manual plugin registration needed:

```rust
use pebble::wgpu::{textures::TextureDescriptor, texture_array::TextureArrayDescriptor, cubemap::CubemapDescriptor};

textures.insert("brick", TextureDescriptor::from_file("assets/brick.png").with_mips());
arrays.insert("atlas", TextureArrayDescriptor::from_files(vec!["a.png", "b.png", "c.png"]));
cubemaps.insert("sky", CubemapDescriptor::from_files(1024, [
    "px.png", "nx.png", "py.png", "ny.png", "pz.png", "nz.png",
]));
```

`from_data`/`from_faces` variants take raw bytes instead of a file path, for procedurally-generated or embedded pixel data. `.with_mips()` (texture only) generates a full mip chain via `MipmapGenerator`. Uploads to `ProcessedAssets<GPUTexture>`/`GPUTextureArray`/`GPUCubemap`, each exposing `texture`/`view`. See [Chapter 9](./ch09-textures.md).

## Samplers

Not built per-use — [`SamplerKind`](../src/wgpu/samplers.rs) picks from a small pre-built cache ([`GlobalSamplers`](../src/wgpu/samplers.rs), set up automatically by `WGPUPlugin`):

```rust
BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)  // in an instance's params
samplers.get(SamplerKind::LinearClamp)                     // -> &wgpu::Sampler, e.g. for a hand-built BindGroupBuilder
```

Variants: `LinearRepeat`, `LinearClamp`, `LinearClampNoMip`, `Nearest`, `NearestClampBorder` (falls back to edge-clamping on web — WebGPU has no border color), `CompareLess` (shadow-map `textureSampleCompare`).

## Custom GPU Resources

Anything one-off that needs the device before it can be built and isn't a material/mesh/texture — a camera, a depth buffer — is a [`LazyResource`](crate::assets::singleton_asset::LazyResource), constructed once `WGPUBackend` (or your own `Deps`) exists, built from the pieces above:

```rust
impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();
    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let bind_group_layout = BindGroupLayoutBuilder::new() /* ... */ .build(&backend.device);
        let buffer = BufferBuilder::new().uniform().size(64).build(&backend.device);
        let bind_group = BindGroupBuilder::new(&bind_group_layout).buffer(&buffer).build(&backend.device);
        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}
```

Full walkthrough, including wiring the resulting layout into a material via `extra_layouts`: [Chapter 10](./ch10-camera-and-depth.md).
