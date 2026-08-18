# Bind Groups and Layouts

A `Material`/`Compute` declares its own bind group (group 0) and supplies its values in the same builder chain — see [Materials](./materials.md)/[Compute Pipelines](./compute-pipelines.md) for the everyday API (`.texture(...)`/`.uniform_value(...)`/etc., or `.with_entry(...)` + a value-only call for anything those don't produce). This page covers the mechanics underneath: what an entry actually is, and how to share or supply a layout beyond group 0.

## Declaring an entry by hand

`.with_entry(name, kind)`/`.with_entry_at(name, binding, kind)` push straight into the same accumulator the streamlined calls do — binding indices auto-increment unless you pin one explicitly with `with_entry_at`:

```rust,ignore
Material::new(SHADER_SOURCE)
    .with_entry("albedo", BindingKind::texture_2d(ShaderStages::FRAGMENT))
    .with_texture("albedo", albedo_handle)
    .with_entry_at("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
    .with_uniform_value("camera", &camera_data)
```

`BindingKind` constructors cover the common cases: `texture_2d`, `texture_2d_array`, `texture_cubemap`, `storage_texture`, `sampler`, `comparison_sampler`, `uniform_buffer`, `dynamic_uniform_buffer`, `storage_buffer_read_only`, `storage_buffer_read_write`, `dynamic_storage_buffer`.

## Sharing a layout across pipelines

`GlobalLayoutPool`, inserted as a resource by `BuiltinAssetsPlugin`, lets unrelated materials/computes share one bind group layout instead of each declaring their own — e.g. a camera uniform every material binds the same way. `.with_extra_group(...)` appends it as group 1 (and up, one call per group, in call order) — group 0 is always the material/compute's own entries:

```rust,ignore
fn register_camera_layout(backend: Read<Backend>, mut pool: Write<GlobalLayoutPool>) {
    let layout = BindGroupLayoutBuilder::new()
        .with_entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
        .build(&backend);
    pool.register("camera", layout);
}

Material::new(SHADER_SOURCE)
    .texture("albedo", albedo_handle)
    .with_extra_group(GroupEntry::Global("camera"))
```

A pipeline can also take a pre-built `BindGroupLayout` directly via `GroupEntry::Layout(layout)` — for a standalone layout that was never registered under a name. Note this opts a `Material`/`Compute` out of [pipeline sharing](./materials.md#many-uniform-combinations-one-shader): an inline layout has no name to structurally compare against another one, so it always compiles its own pipeline.

## Building the actual bind group

Once a `Material`/`Compute` uploads, it produces a `BindGroup` internally — you don't usually build one by hand. If you are (e.g. for [Custom GPU Resources](./custom-gpu-resources.md)), `BindGroupBuilder` matches values to slots in binding order, or explicitly via the `_at(binding, ...)` variant:

```rust,ignore
let bind_group = BindGroupBuilder::new(&layout)
    .with_texture_2d(&gpu_texture)
    .with_sampler(&sampler)
    .with_buffer(&camera_buffer)
    .build(&backend);
```

Also available: `with_texture_array`, `with_texture_cubemap`, `with_texture_view`, `with_dynamic_buffer`.

Reach for `BindGroupBuilder` directly only when you're not going through the asset pipeline at all — see [Custom GPU Resources](./custom-gpu-resources.md). `Material`/`Compute` themselves resolve their own named values (`.with_texture(...)`, `.with_uniform_value(...)`, etc.) against their own declared entries the same way, by name, at upload time — that resolution isn't part of the public API, since there's no longer a generic pipeline type it needs to work against.
