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

Building a layout by hand this way is mostly for resources outside the material/compute system (a camera — see [Custom GPU Resources](./custom-gpu-resources.md)); [`MaterialDescriptor`](./materials.md)/[`ComputeDescriptor`](./compute-pipelines.md) build their own layout internally from `entries`.

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

Internally, `MaterialDescriptor`/`ComputeDescriptor` assemble several already-built `BindGroupLayout`s into the array a pipeline layout needs, keyed by explicit `@group(N)` rather than position, combining their own bind group (`own_group`) with `extra_layouts`. The field to reach for is `extra_layouts` on a material/compute descriptor:

```rust
MaterialDescriptor {
    // ... own entries occupy own_group (default Some(0)) ...
    extra_layouts: vec![OwnedGroupLayout { group: 1, layout: camera.bind_group_layout.clone() }],
    ..Default::default()
}
```

`OwnedGroupLayout::layout` takes the same opaque `BindGroupLayout` a `BindGroupLayoutBuilder` builds — `Clone` because the same layout might be wired into more than one material/compute pass. `own_group` (default `Some(0)`) plus every group in `extra_layouts` must cover `0..=max` exactly once — panics on a gap or a collision, turning a mismatched `@group(N)` in the shader into an immediate, specific error instead of an opaque wgpu validation failure at draw time. See [Custom GPU Resources](./custom-gpu-resources.md#wiring-the-camera-into-a-materials-pipeline-layout) for a worked example wiring a camera's layout into `@group(1)`.
