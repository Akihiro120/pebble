# Materials

A `Material` is a render pipeline asset *and* its own bind group values in one — WGSL shader source, the fixed-function state (vertex layouts, cull mode, depth, targets) needed to compile it, and the textures/samplers/uniforms it renders with. It follows the same [asset pipeline](./the-asset-pipeline.md) pattern as everything else, and produces one `Handle<Material>`:

```rust,ignore
fn setup(backend: Read<Backend>, mut materials: Write<Assets<Material>>, mut textures: Write<Assets<Texture>>) {
    let albedo = Texture::from_file("albedo.png").build_asset("albedo", &mut textures);

    let handle = Material::new(SHADER_SOURCE)
        .with_label("unlit")
        .with_vertex_layouts(vec![Vertex::layout()])
        .texture("albedo", albedo)
        .sampler("albedo_sampler", SamplerKind::LinearRepeat)
        .with_targets(vec![ColorTargetState {
            format: backend.surface_format(),
            blend: Some(BlendState::ALPHA_BLENDING),
            write_mask: ColorWrites::ALL,
        }])
        .build_asset("unlit", &mut materials);
}
```

Defaults: `vs_main`/`fs_main` entry points, back-face culling, fill mode, no depth testing, sample count 1. Override with `with_vertex_entry`/`without_vertex_entry`, `with_cull_mode`/`without_cull_mode`, `with_depth`/`without_depth`, `with_polygon_mode`, `with_sample_count`.

`ColorTargetState::DEFAULT_TARGET` is a ready-made single opaque `Rgba8Unorm` target, if you don't need custom blending. `DepthStencilState::DEFAULT` is likewise a ready-made `Depth32Float`/`Less`/depth-write-enabled state, for the common opaque-3D case. Both section types (`ColorTargetState`, `BlendState`/`BlendComponent`, `DepthStencilState`) also implement plain `Default` (matching these same values), for `..Default::default()` struct-update syntax or generic code — e.g. `ColorTargetState { format: my_format, ..Default::default() }` to override just the format.

## `Material::standard`: presets for the common case

If most of your materials are ordinary opaque 3D geometry using the built-in `Vertex` type, `Material::standard(shader_source)` saves retyping the same three calls every time — it's `Material::new(shader_source)` pre-chained with `.with_vertex_layouts(vec![Vertex::layout()])`, a single opaque target in the *actual* surface format, and `.with_depth(DepthStencilState::DEFAULT)`:

```rust,ignore
let handle = Material::standard(SHADER_SOURCE)
    .with_label("unlit")
    .texture("albedo", albedo)
    .build_asset("unlit", &mut materials);
```

The surface format isn't a hardcoded guess (the real one varies by platform/backend — `Bgra8Unorm` is common on Windows/DX12, not the `Rgba8Unorm` `DEFAULT_TARGET` assumes), and it isn't looked up when you call `standard()` either — `standard()` takes no `Backend` reference, same as `new`. It's resolved against the real `Backend` at upload time instead, the same way a `Texture`'s `MipLevels` gets resolved against the texture's actual size only once that's known, rather than at construction.

It's still a plain builder underneath — chain `.with_vertex_layouts(...)`/`.with_targets(...)`/`.with_depth(...)`/`.without_depth()` afterwards to override any one of the three for a material that doesn't fit the common case (a custom vertex type, a blended target, no depth test). For anything more different than that, start from `Material::new` instead.

## Bind group values: streamlined vs. manual

`.texture(name, handle)` above does two things in one call: it declares a fragment-visible `texture_2d<f32>` entry at the next auto-assigned binding index, *and* binds `handle` to it. The full streamlined set: `.texture`/`.texture_array`/`.cubemap` (a `Handle`, resolved at upload time), `.sampler`, `.uniform`/`.storage` (raw bytes), `.uniform_value`/`.storage_value` (a typed value — see [below](#typed-uniforms-with-encase)).

That covers the common case — one bind group (group 0), everything fragment-visible, binding indices in call order. Drop to the manual, two-step form when you need something it doesn't produce:

```rust,ignore
Material::standard(SHADER_SOURCE)
    // vertex-visible, and pinned to binding 0 explicitly
    .with_entry_at("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
    .with_uniform_value("camera", &camera_data)
    // same idea for anything else with a non-default sample type, dynamic
    // offset, etc. — see Bind Groups and Layouts
    .build_asset("unlit", &mut materials)
```

`.with_entry`/`.with_entry_at` declare the entry; the value-only counterparts (`.with_texture`, `.with_sampler`, `.with_uniform`/`.with_storage`, `.with_uniform_value`/`.with_storage_value`, `.with_buffer`/`.with_dynamic_buffer`) bind the value against whatever was declared — see [Bind Groups and Layouts](./bind-groups.md) for the full picture of how entries and values match up by name.

Names must match between an entry and its value — a mismatch fails to build silently (the upload retries forever, since the binding lookup returns `None`). The uploaded `GPUMaterial` gives you `.update(name, data)` (or `.update_value(name, &value)` for a typed value) to overwrite a bound uniform/storage buffer in place, without rebuilding the whole bind group — handy for a per-frame value like a camera matrix.

### Every value type, and how to add it

Every kind of value a `Material`'s bind group can hold, and every way to add one — streamlined one-call, manual `.with_entry`/`.with_entry_at` + a value-only call, and (where one exists) the [`#[derive(MaterialParams)]`](./material-params-derive.md) field attribute:

| Value | WGSL type | Streamlined | Manual: declare + bind | Derive attribute |
|---|---|---|---|---|
| Texture | `texture_2d<f32>` | `.texture(name, handle)` | `BindingKind::texture_2d(vis)` + `.with_texture(name, handle)` | `#[texture(N)]` |
| Texture array | `texture_2d_array<f32>` | `.texture_array(name, handle)` | `BindingKind::texture_2d_array(vis)` + `.with_texture_array(name, handle)` | `#[texture_array(N)]` |
| Cubemap | `texture_cube<f32>` | `.cubemap(name, handle)` | `BindingKind::texture_cubemap(vis)` + `.with_cubemap(name, handle)` | `#[cubemap(N)]` |
| Pre-built view (one mip, a standalone render target) | `texture_2d<f32>` | — | `BindingKind::texture_2d(vis)` + `.with_texture_view(name, view)` | — |
| Storage texture | `texture_storage_2d<format, access>` | — | `BindingKind::storage_texture(vis, format, access, dim)` + `.with_texture_view(name, view)` (same resource kind as a plain view — the layout entry is what makes it a storage texture) | — |
| Sampler | `sampler` | `.sampler(name, kind)` | `BindingKind::sampler(vis)` + `.with_sampler(name, kind)` | `#[sampler(N)]` |
| Comparison sampler (shadow maps: `SamplerKind::CompareLess`) | `sampler_comparison` | — | `BindingKind::comparison_sampler(vis)` + `.with_sampler(name, SamplerKind::CompareLess)` — `.sampler(...)` always declares a plain (non-comparison) sampler, so this one needs the manual form | — |
| Uniform, raw bytes | `uniform<...>` | `.uniform(name, bytes)` | `BindingKind::uniform_buffer(vis)` + `.with_uniform(name, bytes)` | — |
| Uniform, typed ([`encase`](#typed-uniforms-with-encase)) | `uniform<...>` | `.uniform_value(name, &val)` | `BindingKind::uniform_buffer(vis)` + `.with_uniform_value(name, &val)` | `#[uniform(N)]` |
| Storage, raw bytes | `storage<...>` | `.storage(name, bytes)` (read-only) | `BindingKind::storage_buffer_read_only`/`_read_write(vis)` + `.with_storage(name, bytes)` | — |
| Storage, typed (`encase`) | `storage<...>` | `.storage_value(name, &val)` (read-only) | same `BindingKind`s + `.with_storage_value(name, &val)` | `#[storage(N)]` |
| An existing [`Buffer`](./buffers.md) you already built (e.g. a compute pass's output) | `uniform<...>`/`storage<...>` | — | matching `BindingKind::uniform_buffer`/`storage_buffer_*(vis)` + `.with_buffer(name, buffer)` — no buffer is created, so `buffer` must already carry the matching `BufferUsages::UNIFORM`/`::STORAGE` | — |
| An existing [`DynamicBuffer`](./buffers.md#dynamic-buffers) | `uniform<...>`/`storage<...>` (dynamic offset) | — | `BindingKind::dynamic_uniform_buffer`/`dynamic_storage_buffer(vis, elem_size)` + `.with_dynamic_buffer(name, buffer)` | — |
| A whole extra bind group (group 1+), shared or standalone | — | — | `.with_extra_group(GroupEntry::Global("name"))` / `GroupEntry::Layout(layout)` — see [Bind Groups and Layouts](./bind-groups.md#sharing-a-layout-across-pipelines) | `#[layout("name")]` / `#[layout(param)]` |

The rows with no derive attribute (comparison samplers, pre-built views/storage textures, an existing `Buffer`/`DynamicBuffer`) aren't a limitation on combining with `#[derive(MaterialParams)]` — `.into_material(...)` hands back an ordinary `Material`, so chain the manual `.with_entry(...)` + value call onto the result exactly as you would without the derive.

## Many uniform combinations, one shader

Compiling a `wgpu::RenderPipeline` is the expensive part of building a `Material` — building its bind group is cheap. So several `Material`s that share the same shader and fixed-function state (vertex layout, cull/depth/targets/polygon mode/sample count, and bind group *shape*) automatically compile just once and share the result — `MaterialPipelineCache`, a resource `BuiltinAssetsPlugin` inserts, handles this for you. In practice: "the same shader, several different uniform values" is just several ordinary `Material`s, not a separate instance concept to manage:

```rust,ignore
let red = Material::standard(ENEMY_SHADER)
    .texture("sprite", sheet)
    .uniform_value("tint", &Tint { color: [1.0, 0.2, 0.2, 1.0] })
    .build_asset("enemy_red", &mut materials);
let green = Material::standard(ENEMY_SHADER)   // same shader + shape as `red` → shares its compiled pipeline
    .texture("sprite", sheet)
    .uniform_value("tint", &Tint { color: [0.2, 1.0, 0.2, 1.0] })
    .build_asset("enemy_green", &mut materials);
```

One caveat: a `Material` using `GroupEntry::Layout(...)` (an inline pre-built layout, rather than the streamlined calls or `GroupEntry::Global`) opts out of this cache — it always compiles its own pipeline, same as every `Material` did before the cache existed.

## Typed uniforms with `encase`

`.uniform_value`/`.storage_value` (and `GPUMaterial::update_value`) take any type implementing `encase::ShaderType` — usually a `#[derive(encase::ShaderType)]` struct — instead of a hand-packed `Vec<u8>`, and lay it out with the correct WGSL alignment for you:

```rust,ignore
#[derive(encase::ShaderType)]
struct Tint {
    color: [f32; 4],
}

Material::standard(SHADER_SOURCE).uniform_value("tint", &Tint { color: [1.0, 0.2, 0.2, 1.0] })
```

`glam`'s vector/matrix types (`Vec2`/`Vec3`/`Vec4`/`Mat2`/`Mat3`/`Mat4`) already implement `ShaderType` (via `glam`'s own `encase` feature, which pebble enables) — use them directly in a uniform struct's fields.

## Building the params struct with `#[derive(MaterialParams)]`

For the common case, you don't have to write the `.texture(...)`/`.uniform_value(...)` chain by hand at all — see [Material/Compute Params](./material-params-derive.md) for a struct-derived version of everything on this page.
