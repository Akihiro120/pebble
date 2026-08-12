# Materials

A `Material` is a render pipeline asset — WGSL shader source plus the fixed-function state (vertex layouts, cull mode, depth, targets) needed to compile it. It follows the same [asset pipeline](./the-asset-pipeline.md) pattern as everything else:

```rust,ignore
fn setup(backend: Read<Backend>, mut materials: Write<Assets<Material>>) {
    let handle = Material::new(SHADER_SOURCE)
        .with_label("unlit")
        .with_vertex_layouts(vec![Vertex::layout()])
        .with_entries(vec![/* see Bind Groups and Layouts */])
        .with_targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: Some(BlendState::ALPHA_BLENDING),
            write_mask: ColorWrites::ALL,
        }])
        .build_asset("unlit", &mut materials);
}
```

Defaults: `vs_main`/`fs_main` entry points, back-face culling, fill mode, no depth testing, sample count 1. Override with `with_vertex_entry`/`without_vertex_entry`, `with_cull_mode`/`without_cull_mode`, `with_depth`, `with_polygon_mode`, `with_sample_count`.

`ColorTargetState::DEFAULT_TARGET` is a ready-made single opaque `Rgba8Unorm` target, if you don't need custom blending.

## Supplying bind group values: `MaterialInstance`

A `Material` only declares what its bind group *shape* is. The actual textures/samplers/uniforms come from a separate asset, `MaterialInstance` (a type alias for `BindingInstance<Material>`):

```rust,ignore
fn make_instance(mut materials: Write<Assets<Material>>, mut instances: Write<Assets<MaterialInstance>>) {
    let material_handle = /* from above */;
    MaterialInstance::new(material_handle)
        .with_texture("albedo", texture_handle)
        .with_sampler("albedo_sampler", SamplerKind::LinearRepeat)
        .with_uniform("camera", bytemuck::bytes_of(&camera_data).to_vec())
        .build_asset("player_instance", &mut instances);
}
```

Names must match the names given in `with_entries` (see [Bind Groups and Layouts](./bind-groups.md)) — a mismatch fails to build silently (the upload retries forever, since the binding lookup returns `None`). The uploaded `GPUMaterialInstance` gives you `update(name, data)` to overwrite a bound uniform/storage buffer in place, without rebuilding the whole bind group — handy for a per-frame value like a camera matrix.
