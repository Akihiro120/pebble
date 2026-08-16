# Bind Groups and Layouts

A material or compute pipeline declares what its shader expects via a list of `GroupEntry`s, then a `BindingInstance` ([Materials](./materials.md) or [Compute Pipelines](./compute-pipelines.md)) supplies the actual values by name.

## Declaring a pipeline's own entries

`OwnEntriesBuilder` builds the `GroupEntry::Own` list for a pipeline's own bind group — binding indices auto-increment unless you pin one explicitly with `with_entry_at`:

```rust,ignore
let entries = OwnEntriesBuilder::new()
    .with_entry("albedo", BindingKind::texture_2d(ShaderStages::FRAGMENT))
    .with_entry("albedo_sampler", BindingKind::sampler(ShaderStages::FRAGMENT))
    .with_entry("camera", BindingKind::uniform_buffer(ShaderStages::VERTEX))
    .build();

Material::new(SHADER_SOURCE).with_entries(vec![entries])
```

`BindingKind` constructors cover the common cases: `texture_2d`, `texture_2d_array`, `texture_cubemap`, `storage_texture`, `sampler`, `comparison_sampler`, `uniform_buffer`, `dynamic_uniform_buffer`, `storage_buffer_read_only`, `storage_buffer_read_write`, `dynamic_storage_buffer`.

## Sharing a layout across pipelines

`GlobalLayoutPool`, inserted as a resource by `BuiltinAssetsPlugin`, lets unrelated materials/computes share one bind group layout instead of each declaring their own — e.g. a camera uniform every material binds the same way:

```rust,ignore
fn register_camera_layout(backend: Read<Backend>, mut pool: Write<GlobalLayoutPool>) {
    let layout = BindGroupLayoutBuilder::new()
        .with_entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
        .build(&backend);
    pool.register("camera", layout);
}

Material::new(SHADER_SOURCE).with_entries(vec![
    OwnEntriesBuilder::new() /* ... */ .build(),
    GroupEntry::Global("camera"),
])
```

A pipeline can also take a pre-built `BindGroupLayout` directly via `GroupEntry::Layout(layout)`.

## Building the actual bind group

Once a `BindingInstance` uploads, it produces a `BindGroup` internally — you don't usually build one by hand. If you are (e.g. for [Custom GPU Resources](./custom-gpu-resources.md)), `BindGroupBuilder` matches values to slots in binding order, or explicitly via the `_at(binding, ...)` variant:

```rust,ignore
let bind_group = BindGroupBuilder::new(&layout)
    .with_texture_2d(&gpu_texture)
    .with_sampler(&sampler)
    .with_buffer(&camera_buffer)
    .build(&backend);
```

Also available: `with_texture_array`, `with_texture_cubemap`, `with_texture_view`, `with_dynamic_buffer`.

## `BindingInstance` covers the same resource kinds

`MaterialInstance`/`ComputeInstance` (`BindingInstance<T>`, see [Materials](./materials.md)) expose a `.with_*(name, ...)` for every kind `BindGroupBuilder` accepts, matched by name instead of binding order: `.with_texture`/`.with_texture_array`/`.with_cubemap` (a `Handle`, resolved at upload time), `.with_texture_view` (an already-built `TextureView`, bound directly — e.g. one mip level from `GPUTexture::get_view`), `.with_sampler`, `.with_buffer`/`.with_dynamic_buffer` (an existing `Buffer`/`DynamicBuffer`), and `.with_uniform`/`.with_storage` (raw bytes — builds a fresh `Buffer` for you). Reach for `BindGroupBuilder` directly only when you're not going through the asset pipeline at all (see [Custom GPU Resources](./custom-gpu-resources.md)).
