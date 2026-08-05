# wgpu API Reference

A task-indexed lookup, not a tutorial — "I need a buffer, what do I call?" For the concepts behind any of this (why materials and instances are separate, why bind group entries need explicit visibility, ...), see the relevant chapter in [Part II](./ch06-assets-and-handles.md); this page just gets you to working code fast. Everything below is reachable via `use pebble::wgpu::prelude::*;` unless noted otherwise.

## Buffers

Every buffer type below is opaque — there's no way to reach a raw `wgpu::Buffer` from outside the crate. Binding one into a bind group goes through [`BindGroupBuilder`](#a-bind-group) directly; writing to one is a method call, not a `queue.write_buffer(...)` you have to thread a queue reference to.

### A plain, uniform, or storage buffer

[`BufferBuilder`](../src/wgpu/buffers.rs) — empty (write into it later) or pre-populated. Takes `&WGPUBackend`, not just a device, since the resulting [`Buffer`](../src/wgpu/buffer.rs) caches its own queue access:

```rust
// Empty, written into later via `.write()`.
let camera_buffer = BufferBuilder::new().label("camera").uniform().size(64).build(&backend);

// Pre-populated.
let vertex_buffer = BufferBuilder::new()
    .label("mesh vertices")
    .usage(BufferUsages::VERTEX)
    .data(bytemuck::cast_slice(&vertices))
    .build(&backend);

// Later, any time:
camera_buffer.write(&new_matrix_bytes);              // whole buffer, offset 0
camera_buffer.write_at(offset, &partial_bytes);       // starting at a byte offset
```

`.uniform()`/`.storage()` are shorthand for the usual `UNIFORM | COPY_DST`/`STORAGE | COPY_DST` flag pairs; use `.usage(...)` directly for anything else (vertex/index buffers, a `MAP_READ` staging buffer).

### A dynamically-offset buffer (many elements, one buffer)

[`DynamicBufferBuilder`](../src/wgpu/buffers.rs) — sized and aligned for `count` elements of `element_size` bytes, selected later via `set_bind_group`'s dynamic offset. Returns a [`DynamicBuffer`](../src/wgpu/buffer.rs) bundling the buffer with its own stride and element size, so neither can drift out of sync with what it was actually built with:

```rust
let dynamic = DynamicBufferBuilder::uniform(element_size, count).build(&backend);
// ... later, per element:
dynamic.write_element(index, &element_bytes);
// ... at draw time:
pass.set_bind_group(0, &bind_group, &[index as u32 * dynamic.stride() as u32]);
```

Pair with [`BindingKind::dynamic_uniform_buffer`](#a-bind-group-layout) for the layout and [`BindGroupBuilder::dynamic_buffer`](#a-bind-group) for the bind group.

## Bind Groups

### A bind group layout

[`BindGroupLayoutBuilder`](../src/wgpu/binding.rs) — one [`BindingKind`](../src/wgpu/binding.rs) per entry, visibility always explicit:

```rust
let layout = BindGroupLayoutBuilder::new()
    .label("camera_layout")
    .entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
    .build(&backend);
```

`BindingKind` constructors, one per resource shape: `texture_2d`/`texture_2d_array`/`texture_cubemap`, `storage_texture`, `sampler`/`comparison_sampler`, `uniform_buffer`/`dynamic_uniform_buffer`, `storage_buffer_read_only`/`storage_buffer_read_write`/`dynamic_storage_buffer` — every one takes `ShaderStages` explicitly, nothing defaulted. Building a layout by hand this way is mostly for resources outside the material/compute system (a camera — see [Chapter 10](./ch10-camera-and-depth.md)); `MaterialDescriptor`/`ComputeDescriptor` below build their own layout internally from `entries`.

`.build()` panics on a duplicate `@binding(N)` — a shader/layout mismatch fails loudly here instead of at draw time.

### A bind group

[`BindGroupBuilder`](../src/wgpu/buffers.rs) — one resource per binding, against an already-built layout. Each resource kind has its own method, taking the matching opaque type directly:

```rust
let bind_group = BindGroupBuilder::new(&layout)
    .label("camera_bind_group")
    .buffer(&camera_buffer)          // &Buffer, @binding(0), call order
    .build(&backend);

// The other resource kinds, same call-order-assigns-@binding(N) shape:
BindGroupBuilder::new(&layout)
    .texture_2d(&brick_texture)      // &GPUTexture
    .texture_array(&atlas)           // &GPUTextureArray
    .texture_cubemap(&sky)           // &GPUCubemap
    .texture_view(&shadow_map)       // &TextureView — a render target, sampled back (see below)
    .sampler(&sampler)               // &Sampler, from GlobalSamplers::get
    .dynamic_buffer(&dynamic)        // &DynamicBuffer
    .build(&backend);
```

`texture_view`/`texture_view_at` is how a render target sampled back in a later pass gets bound — a shadow map read in the lighting pass, a post-process input, an offscreen pass fed into a full-screen quad. It takes the same opaque [`TextureView`](#a-render-target--depth-buffer-no-source-data) that [`TextureBuilder::build`](#a-render-target--depth-buffer-no-source-data)/[`GPUCubemap::face_attachment`](#rendering-into-a-cubemap-face-environment-capture) hand back — build it with both `RENDER_ATTACHMENT` (to render into it) and `TEXTURE_BINDING` (to sample it) usage:

```rust
let shadow_map = TextureBuilder::new(2048, 2048, TextureFormat::Depth32Float)
    .usage(TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING)
    .build(&backend);

// Sampling a depth texture (rather than a color one) as `texture_depth_2d` in
// WGSL needs `TextureSampleType::Depth` in the layout entry, and typically a
// comparison sampler (`textureSampleCompare`) rather than a regular one —
// spell the full BindingKind::Texture variant out rather than reaching for
// `texture_2d`, which defaults to a filterable float sample type:
BindingEntry {
    name: "shadow_map",
    binding: 0,
    kind: BindingKind::Texture {
        visibility: ShaderStages::FRAGMENT,
        sample_type: TextureSampleType::Depth,
        view_dimension: TextureViewDimension::D2,
        multisampled: false,
    },
};
BindingKind::comparison_sampler(ShaderStages::FRAGMENT); // paired sampler entry
```

If your bindings aren't contiguous from 0 (looked up by name, as material/compute instances are — see below), use the `_at` variants instead (`.buffer_at(2, &buf)`, `.texture_2d_at(0, &tex)`, ...).

## Pipeline Layouts (multiple bind groups)

Internally, `MaterialDescriptor`/`ComputeDescriptor` assemble several already-built [`BindGroupLayout`](#a-bind-group-layout)s into the array a pipeline layout needs, keyed by explicit `@group(N)` rather than position, combining their own bind group (`own_group`) with `extra_layouts`. The field to reach for is `extra_layouts` on a material/compute descriptor:

```rust
MaterialDescriptor {
    // ... own entries occupy own_group (default Some(0)) ...
    extra_layouts: vec![OwnedGroupLayout { group: 1, layout: camera.bind_group_layout.clone() }],
    ..Default::default()
}
```

`OwnedGroupLayout::layout` takes the same opaque `BindGroupLayout` a `BindGroupLayoutBuilder` builds — `Clone` because the same layout might be wired into more than one material/compute pass. Panics on a gap or a collision in `0..=max` across `own_group` + `extra_layouts` combined — see [Chapter 10](./ch10-camera-and-depth.md#wiring-the-camera-into-the-materials-pipeline-layout).

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
    targets: vec![ColorTargetState {
        format: backend.surface_format(),
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

`BindingInstanceEntry` variants: `Texture`/`TextureArray`/`Cubemap`/`Sampler` (all by handle/kind, resolved against existing processed assets) and `Uniform(Vec<u8>)`/`Storage(Vec<u8>)` (a buffer the instance allocates and owns itself, updatable later via `instance.update(name, &bytes)`, or read back to the CPU via `instance.buffer(name).read()`/`read_as::<T>()` — see [Chapter 11](./ch11-compute.md#reading-the-result-back)). Full walkthrough: [Chapter 9](./ch09-textures.md).

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
        kind: BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE),
    }],
    ..Default::default()
});
```

Dispatching isn't `FrameOperations`-mediated — a compute pass isn't tied to an acquired frame the way a render pass is — but it's just as opaque; see [Compute Dispatch](#compute-dispatch) below.

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

## Rendering (Pass Recording)

[`RenderPass`](../src/wgpu/render_pass.rs) — what `ActiveFrame::begin_pass`/`render_context` hand back, opaque like everything else here: no raw `wgpu::RenderPass` anywhere, draw-time operations are methods instead:

```rust
let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);
pass.set_pipeline(&material.pipeline);       // &RenderPipeline
pass.set_bind_group(0, &instance.bind_group, &[]); // &BindGroup, dynamic offsets last
pass.set_vertex_buffer(0, &mesh.vertex_buffer);    // &Buffer, whole buffer
pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32); // &Buffer + IndexFormat
pass.draw_indexed(0..mesh.index_count, 0, 0..1);
pass.draw(0..vertex_count, 0..1);            // non-indexed
```

`IndexFormat` (`Uint16`/`Uint32`) mirrors `wgpu::IndexFormat` — the same two variants, just not the raw type. See [Chapter 8](./ch08-first-triangle.md) (no bind group), [Chapter 9](./ch09-textures.md) (with an instance's bind group), [Chapter 10](./ch10-camera-and-depth.md) (a depth attachment + a second bind group).

### Indirect draws

`draw_indirect`/`draw_indexed_indirect` read the vertex/instance counts from a buffer instead of a CPU-known value — for a draw count the GPU itself computed (culling compaction, LOD selection, ...):

```rust
let args = DrawIndirectArgs { vertex_count: 3, instance_count: 1, first_vertex: 0, first_instance: 0 };
let indirect_buffer = BufferBuilder::new().usage(BufferUsages::INDIRECT | BufferUsages::COPY_DST).data(args.as_bytes()).build(&backend);

pass.draw_indirect(&indirect_buffer, 0);          // reads a DrawIndirectArgs at byte offset 0
pass.draw_indexed_indirect(&indirect_buffer, 0);  // reads a DrawIndexedIndirectArgs instead
```

`DrawIndirectArgs`/`DrawIndexedIndirectArgs` mirror `wgpu`'s own argument layouts field-for-field — typically written by a compute shader into a storage buffer (also usable as `INDIRECT`) rather than built on the CPU like the example above.

## Multisampled Anti-Aliasing (MSAA)

Turning on MSAA for the window surface is a `WGPUBackend` setting, not something you wire into `Pass`/`ColorTarget` by hand — call it once at startup, before building any material that renders into the default target:

```rust
fn setup_msaa(mut backend: ResMut<WGPUBackend>) {
    backend.set_msaa(4); // 0 or 1 turns it back off
}
```

Once set, `ColorTarget::Default` automatically renders into an internally-managed multisampled texture and resolves into the real surface — [`WGPUBackend::resize`](../src/wgpu/backend.rs) keeps that texture matched to the surface size, no extra wiring needed on your end. Two things still need to opt in explicitly, because a single frame can legitimately mix sample counts (an MSAA scene pass, then a non-MSAA post-process/UI pass reading the resolved result — forcing every material to match one global count would break exactly that):

```rust
// A material meant to render into the (now-MSAA) default target:
MaterialDescriptor {
    sample_count: backend.sample_count(), // 1 (the MaterialDescriptor default) means "not this pass"
    ..Default::default()
}

// A depth attachment used alongside it needs the same sample count:
TextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth32Float)
    .sample_count(backend.sample_count())
    .usage(TextureUsages::RENDER_ATTACHMENT)
    .build(&backend);
```

A post-process/UI material rendering into the already-resolved surface (or any other non-MSAA target) just leaves `sample_count` at its default of `1`.

## Render Bundles

[`RenderBundleEncoder`](../src/wgpu/render_bundle.rs) records a reusable sequence of draw calls once, replayed via [`RenderPass::execute_bundles`](../src/wgpu/render_pass.rs) — worth it once you have many draws that don't change pipeline/bind group/buffers from one frame to the next (static scene geometry, say), since replaying a bundle is typically cheaper than re-recording the same commands by hand every frame:

```rust
let mut encoder = backend.create_render_bundle_encoder(&RenderBundleEncoderDescriptor {
    color_formats: vec![Some(backend.surface_format())],
    depth_stencil_format: Some(TextureFormat::Depth32Float), // None if this pass has no depth attachment
    sample_count: backend.sample_count(), // must match the pass(es) it's executed in, same as MSAA above
    ..Default::default()
});
encoder.set_pipeline(&material.pipeline);
encoder.set_bind_group(0, &instance.bind_group, &[]);
encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
encoder.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
encoder.draw_indexed(0..mesh.index_count, 0, 0..1);
let bundle = encoder.finish(Some("static-geometry"));

// ... later, once per frame, inside an ordinary render pass:
pass.execute_bundles(&[&bundle]);
```

`color_formats`/`depth_stencil_format`/`sample_count` must match whatever render pass(es) the bundle is later executed against, or wgpu's validation rejects it — the same requirement as a material's own `targets`/`depth`/`sample_count`.

## Compute Dispatch

A compute pass isn't tied to an acquired frame, so it needs its own encoder — [`WGPUBackend::create_command_encoder`](../src/wgpu/backend.rs)/[`CommandEncoder::compute_pass`](../src/wgpu/compute_pass.rs)/[`WGPUBackend::submit`](../src/wgpu/backend.rs) cover that the same opaque way `begin_pass` covers rendering:

```rust
let mut encoder = backend.create_command_encoder(Some("double-encoder"));
{
    let mut compute_pass = encoder.compute_pass(Some("double-pass"));
    compute_pass.set_pipeline(&pass.pipeline);            // &ComputePipeline
    compute_pass.set_bind_group(0, &instance.bind_group, &[]);
    compute_pass.dispatch_workgroups(1, 1, 1);
}
backend.submit(encoder);
```

`compute_pass.dispatch_workgroups_indirect(&indirect_buffer, offset)` is the compute-side equivalent of [indirect draws](#indirect-draws) — reads a `DispatchIndirectArgs { x, y, z }` from `indirect_buffer` at `offset` instead of a CPU-known workgroup count.

See [Chapter 11](./ch11-compute.md).

## Textures

Three descriptors, same `from_*` constructor pattern — decode/upload happens on the asset pipeline like any other asset, no manual plugin registration needed. `TextureDescriptor`'s `format` isn't limited to the four channel-count-4 formats you'll see in the examples below — every regular 8/16/32-bit unorm/float format (`R8Unorm`, `Rg16Float`, `Bgra8Unorm`, ...) works for both `from_file` decoding and raw `from_data` uploads. Block-compressed formats (`Bc*`/`Etc2*`/`Astc`) aren't supported by either path yet — decoding one from an ordinary image file isn't possible this way regardless, and uploading pre-compressed bytes via `from_data` needs block-aware row-stride math this loader doesn't compute yet:

```rust
use pebble::wgpu::{textures::TextureDescriptor, texture_array::TextureArrayDescriptor, cubemap::CubemapDescriptor};

textures.insert("brick", TextureDescriptor::from_file("assets/brick.png").with_mips());
arrays.insert("atlas", TextureArrayDescriptor::from_files(vec!["a.png", "b.png", "c.png"]));
cubemaps.insert("sky", CubemapDescriptor::from_files(1024, [
    "px.png", "nx.png", "py.png", "ny.png", "pz.png", "nz.png",
]));
```

`from_data`/`from_faces` variants take raw bytes instead of a file path, for procedurally-generated or embedded pixel data. `.with_mips()` (texture only) generates a full mip chain via `MipmapGenerator`. Uploads to `ProcessedAssets<GPUTexture>`/`GPUTextureArray`/`GPUCubemap` — all three opaque, no `texture`/`view` fields to reach in from outside the crate. Each exposes `.width()`/`.height()` (or `.size()` for a cubemap) and a `.write*()` to overwrite level-0 pixel data after upload:

```rust
brick_texture.write(&new_pixels);              // GPUTexture
atlas.write_layer(2, &new_pixels);              // GPUTextureArray, one layer
sky.write_face(0, &new_pixels);                 // GPUCubemap, one face (+X)
```

See [Chapter 9](./ch09-textures.md).

### Rendering into a cubemap face (environment capture)

[`GPUCubemap::face_attachment`](../src/wgpu/cubemap.rs) — a render-target [`TextureView`](#a-render-target--depth-buffer-no-source-data) onto one face (`0..=5`, `+X -X +Y -Y +Z -Z`) at one mip level, for a `CubemapDescriptor::empty()` cubemap (which sets `RENDER_ATTACHMENT` usage automatically). Capture a scene into all 6 faces, or write successive mip levels from a specular IBL prefilter pass:

```rust
for face in 0..6 {
    let view = cubemap.face_attachment(face, 0);
    let mut pass = active.begin_pass(Pass {
        colors: &[ColorTarget::Custom { attachment: &view, clear: Some([0.0, 0.0, 0.0, 1.0]) }],
        depth: None,
    });
    // ... render the scene from this face's view direction ...
}
```

### A render target / depth buffer (no source data)

[`TextureBuilder`](../src/wgpu/texture_view.rs) — for a one-off GPU-side texture with nothing to upload (a depth buffer, an off-screen render target), unlike `TextureDescriptor` above which always loads from a file/bytes through the asset pipeline. Hands back an opaque [`TextureView`](../src/wgpu/texture_view.rs) — the type `ActiveFrame::begin_pass`'s `ColorTarget::Custom`/`DepthTarget` expect:

```rust
let depth_view = TextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
    .label("depth")
    .usage(TextureUsages::RENDER_ATTACHMENT)
    .build(backend);

// ... later, in a render system:
let mut pass = active.begin_pass(Pass {
    colors: &[ColorTarget::default([0.2, 0.3, 0.3, 1.0])],
    depth: Some(DepthTarget::new(&depth_view, 1.0)),
});
```

See [Chapter 10](./ch10-camera-and-depth.md).

## Samplers

Not built per-use — [`SamplerKind`](../src/wgpu/samplers.rs) picks from a small pre-built cache ([`GlobalSamplers`](../src/wgpu/samplers.rs), set up automatically by `WGPUPlugin`):

```rust
BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)  // in an instance's params
samplers.get(SamplerKind::LinearClamp)                     // -> &Sampler, e.g. for a hand-built BindGroupBuilder
```

Variants: `LinearRepeat`, `LinearClamp`, `LinearClampNoMip`, `Nearest`, `NearestClampBorder` (falls back to edge-clamping on web — WebGPU has no border color), `CompareLess` (shadow-map `textureSampleCompare`).

## Custom GPU Resources

Anything one-off that needs the device before it can be built and isn't a material/mesh/texture — a camera — is a [`LazyResource`](crate::assets::singleton_asset::LazyResource), constructed once `WGPUBackend` (or your own `Deps`) exists, built from the pieces above:

```rust
impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();
    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let bind_group_layout = BindGroupLayoutBuilder::new() /* ... */ .build(backend);
        let buffer = BufferBuilder::new().uniform().size(64).build(backend);
        let bind_group = BindGroupBuilder::new(&bind_group_layout).buffer(&buffer).build(backend);
        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}
```

Every field here — `bind_group_layout`, `buffer`, `bind_group` — is opaque, same as everywhere else in `pebble::wgpu`: no `wgpu::*` type anywhere in `Camera`'s own definition.

Full walkthrough, including wiring the resulting layout into a material via `extra_layouts`: [Chapter 10](./ch10-camera-and-depth.md).
