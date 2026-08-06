# Bind Groups and Layouts

## A bind group layout

[`BindGroupLayoutBuilder`](../src/wgpu/binding.rs) — one [`BindingKind`](../src/wgpu/binding.rs) per entry, visibility always explicit:

```rust
let layout = BindGroupLayoutBuilder::new()
    .label("camera_layout")
    .entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
    .build(&backend);
```

`BindingKind` constructors, one per resource shape: `texture_2d`/`texture_2d_array`/`texture_cubemap`, `storage_texture`, `sampler`/`comparison_sampler`, `uniform_buffer`/`dynamic_uniform_buffer`, `storage_buffer_read_only`/`storage_buffer_read_write`/`dynamic_storage_buffer` — every one takes `ShaderStages` explicitly, nothing defaulted. Visibility is explicit rather than inferred because `BindingKind` is shared between materials and [compute passes](./compute-pipelines.md) — a material entry can be `FRAGMENT`/`VERTEX`/`VERTEX_FRAGMENT`, a compute entry must be exactly `COMPUTE`, and building a material or compute pipeline panics if an entry's visibility doesn't fit, catching the mistake immediately instead of deep inside a wgpu validation error.

Building a layout by hand this way is mostly for resources outside the material/compute system (a camera — see [Custom GPU Resources](./custom-gpu-resources.md)); [`Material`](./materials.md)/[`Compute`](./compute-pipelines.md) build their own layout internally from a `GroupEntry::Own` entry's `BindingEntry`s.

`.build()` panics on a duplicate `@binding(N)` — a shader/layout mismatch fails loudly here instead of at draw time.

## A bind group

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

`texture_view`/`texture_view_at` is how a render target sampled back in a later pass gets bound — a shadow map read in the lighting pass, a post-process input, an offscreen pass fed into a full-screen quad. It takes the same opaque [`TextureView`](./textures.md#a-render-target--depth-buffer-no-source-data) that [`TextureBuilder::build`](./textures.md#a-render-target--depth-buffer-no-source-data)/[`GPUCubemap::face_attachment`](./textures.md#rendering-into-a-cubemap-face-environment-capture) hand back — build it with both `RENDER_ATTACHMENT` (to render into it) and `TEXTURE_BINDING` (to sample it) usage:

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

If your bindings aren't contiguous from 0 (looked up by name, as material/compute instances are — see [Materials](./materials.md#a-material-instance-concrete-resources-bound-to-a-material)), use the `_at` variants instead (`.buffer_at(2, &buf)`, `.texture_2d_at(0, &tex)`, ...).

## Pipeline layouts (multiple bind groups)

A material/compute's whole pipeline layout — its own bind group plus anything external — is one call: [`.entries(Vec<GroupEntry>)`](../src/wgpu/layout.rs). Position in that list *is* the `@group(N)` index, so there's no separate group number to keep in sync with the shader by hand — the first element is `@group(0)`, the second `@group(1)`, and so on:

```rust
use pebble::wgpu::layout::GroupEntry;

Material::new(SHADER)
    .entries(vec![
        GroupEntry::Global("camera"),         // @group(0): pulled from the global pool by name
        GroupEntry::Own(material_entries()),  // @group(1): this material's own texture/sampler
    ])
    // ...
    .build_asset("lit", &mut materials);
```

`GroupEntry` has three variants:

- **`Own(Vec<BindingEntry>)`** — this material/compute's own bind group entries, built into a fresh layout internally. At most one of these is allowed per `.entries(...)` list — `build_material`/`build_compute` panics on a second one, since there's only one instance-bindable group per material/compute (the one a `MaterialInstance`/`ComputeInstance` binds concrete resources against).
- **`Global(&'static str)`** — a layout looked up by name in the [`GlobalLayoutPool`](#a-pool-of-shared-layouts) resource. Resolved lazily, at upload time — the material/compute doesn't need `name` to already be registered while `.entries(...)` is being called, only by the time it actually uploads (if it's not there yet, upload quietly retries next tick, same as any other unmet dependency). This is the normal way to reach a camera, lights, or anything else shared across many materials.
- **`Layout(BindGroupLayout)`** — an already-built layout occupying this position directly, for anything that isn't going through the pool (built by hand — see [Custom GPU Resources](./custom-gpu-resources.md)). `Clone` because the same layout might be wired into more than one material/compute pass this way.

`build_material`/`build_compute` also panics if `.entries(...)` needs more bind groups than the device's `max_bind_groups` allows (`wgpu` guarantees only 4, `@group(0..=3)`) — turning that mistake into an immediate, specific error instead of an opaque wgpu validation failure at draw time.

### A pool of shared layouts

[`GlobalLayoutPool`](../src/wgpu/layout.rs) is a named `name -> BindGroupLayout` registry for layouts shared across many materials/compute passes (a camera, lights, ...) — inserted empty as a resource by `WGPUPlugin`, so it's always there, and pulled in automatically as a `Deps` by every material/compute's `Asset::upload`. Register into it from wherever the layout becomes ready (typically a follow-up system with `ResMut<GlobalLayoutPool>`, once its source `LazyResource` exists):

```rust
fn register_camera_layout(camera: Res<Camera>, mut pool: ResMut<GlobalLayoutPool>) -> Option<()> {
    pool.register("camera", camera.bind_group_layout.clone());
    Some(())
}
```

Order relative to whatever `setup` system builds the material doesn't matter — `GroupEntry::Global("camera")` doesn't need `"camera"` to be registered yet when `.entries(...)` is called, only by the time the material actually uploads, so `setup` doesn't even need `Res<Camera>` as a dependency anymore (only whatever *renders* with the camera's bind group still does). A material that doesn't need every registered global just doesn't reference it by name, so its pipeline layout doesn't carry a group it never uses. See [Custom GPU Resources](./custom-gpu-resources.md#wiring-the-camera-into-a-materials-pipeline-layout) for a worked example wiring a camera's layout into `@group(1)`.
