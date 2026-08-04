# Textures and Material Instances

The previous chapter's material had no bind group at all — nothing to give the shader beyond raw vertex data. Sampling a texture means changing three things: the shader gains a `@group`, the material declares what that group contains, and something has to supply an *actual* texture + sampler for that group at draw time. That third piece is a new concept: a **material instance**.

This chapter's full code is the `wgpu_showcase` example verified alongside this book — run it yourself with `cargo run` from `examples/wgpu_showcase`.

## Why materials and instances are separate

A `GPUMaterial` is a pipeline — compiled once, describing *what shape* of bind group a shader expects (a texture at binding 0, a sampler at binding 1). It says nothing about *which* texture. That's deliberate: the same brick-wall material should be reusable for a floor and a crate without recompiling a pipeline for each — only the bound texture differs. A `GPUMaterialInstance` is that missing piece: a concrete bind group, built by resolving a material's declared entries against actual assets.

## The shader, now with a texture

```rust
const SHADER: &str = r#"
struct VOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var out: VOut;
    out.clip_pos = vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var albedo: texture_2d<f32>;
@group(0) @binding(1) var albedo_sampler: sampler;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(albedo, albedo_sampler, in.uv);
}
"#;
```

## Declaring the bind group on the material

`BindingEntry`/`BindingKind` describe *the shape* of `@group(0)` — a texture at binding 0, a sampler at binding 1, both fragment-visible:

```rust
use pebble::wgpu::binding::{BindingEntry, BindingKind};

fn material_entries() -> Vec<BindingEntry> {
    vec![
        BindingEntry {
            name: "albedo",
            binding: 0,
            kind: BindingKind::texture_2d(wgpu::ShaderStages::FRAGMENT),
        },
        BindingEntry {
            name: "albedo_sampler",
            binding: 1,
            kind: BindingKind::sampler(wgpu::ShaderStages::FRAGMENT),
        },
    ]
}
```

Visibility is explicit on every entry rather than inferred — `BindingKind` is shared between materials and [compute passes](./ch11-compute.md), and `build_material` panics if any entry here were accidentally `COMPUTE`-visible instead of catching the mistake deep inside a wgpu validation error. `name` is purely a diagnostic label matched against instance params below — it has no effect on the actual binding, which is entirely positional (`binding: N`).

With entries non-empty, `MaterialDescriptor` also needs `own_group: Some(0)` (the default) instead of `None` — this is what tells `build_material` these entries occupy `@group(0)` in the pipeline layout, rather than there being no bind group at all.

## Loading a texture

```rust
use pebble::wgpu::textures::TextureDescriptor;

let brick = textures.insert(
    "brick",
    TextureDescriptor::from_file("../assets/textures/brick.png").with_mips(),
);
```

`Assets<TextureDescriptor>` and its `ProcessedAssets<GPUTexture>` counterpart are registered automatically by `WGPUPlugin`, same as mesh and material — decoding and uploading happen on `AssetSync` like any other asset.

## Binding it: the material instance

```rust
use pebble::wgpu::{
    instance::{BindingInstanceEntry, MaterialInstanceDescriptor},
    samplers::SamplerKind,
};

let brick_instance = instances.insert(
    "brick_instance",
    MaterialInstanceDescriptor::new(
        material.id,
        vec![
            ("albedo", BindingInstanceEntry::Texture(brick.id)),
            ("albedo_sampler", BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)),
        ],
    ),
);
```

Each `(name, BindingInstanceEntry)` pair is matched against the material's own `BindingEntry::name`s to find the right `@binding(N)` — the names here (`"albedo"`, `"albedo_sampler"`) must match the ones in `material_entries()` above, or the instance fails to upload. `MaterialInstanceDescriptor::new` takes `RawAssetHandle`s, not typed `Handle<T>`s — that's `material.id`/`brick.id`, unwrapping the typed handles. This is the one place `RawAssetHandle` shows up directly (see Chapter 6): an instance crosses between the *material's* `ProcessedAssets<GPUMaterial>` and the *texture's* `ProcessedAssets<GPUTexture>`, two different `T`s that no single typed `Handle<T>` could refer to at once.

`SamplerKind::LinearRepeat` pulls from a small global cache of common sampler configurations (`GlobalSamplers`, set up automatically by `WGPUPlugin`) rather than creating a new `wgpu::Sampler` per instance — samplers are cheap to share and there's rarely a reason not to.

## Spawning and rendering

```rust
commands.spawn((quad, brick_instance)); // Handle<MeshDescriptor>, Handle<MaterialInstanceDescriptor>
```

```rust
use pebble::wgpu::instance::GPUMaterialInstance;
use pebble::wgpu::render_pass::IndexFormat;

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<MeshDescriptor>, &Handle<MaterialInstanceDescriptor>)>,
) {
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for (mesh_handle, instance_handle) in query.iter() {
        let Some(mesh) = meshes.get(mesh_handle.id) else { continue };
        let Some(instance) = instances.get(instance_handle.id) else { continue };
        let Some(material) = materials.get(instance.target) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, &instance.bind_group, &[]);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
```

Two changes from Chapter 8's `render`: an extra `Res<ProcessedAssets<GPUMaterialInstance>>`, an extra `pass.set_bind_group(0, ...)` call, and the query now looks up the material *through* the instance (`instance.target`, a `RawAssetHandle`) instead of holding a material handle on the entity directly. The entity itself only needs to know its mesh and its instance — the instance already knows which material it belongs to.

Run `wgpu_showcase` and you get a brick-textured quad. The [uniform/storage buffer variants](../src/wgpu/instance.rs) of `BindingInstanceEntry` — for a per-instance color tint, say — follow the exact same `(name, entry)` shape, just with `Uniform(bytes)`/`Storage(bytes)` instead of `Texture(handle)`.
