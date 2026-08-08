# Materials

A `GPUMaterial` is a render pipeline — compiled once, describing *what shape* of bind group a shader expects (a texture at binding 0, a sampler at binding 1). It says nothing about *which* texture. That's deliberate: the same brick-wall material should be reusable for a floor and a crate without recompiling a pipeline for each — only the bound texture differs. A `GPUMaterialInstance` is that missing piece: a concrete bind group, built by resolving a material's declared entries against actual assets.

## Building a material

[`Material`](../src/wgpu/material.rs) is plain data with no public constructors — the only way to build one is [`MaterialBuilder`](../src/wgpu/material.rs): chain setters, then `.build_asset(name, &mut assets)` (or `.build()` for a value you insert yourself), uploaded automatically to `ProcessedAssets<GPUMaterial>` (registered by `WGPUPlugin`, no manual plugin needed):

```rust
use pebble::wgpu::{binding::{BindingEntry, BindingKind}, layout::GroupEntry, material::MaterialBuilder, mesh::Vertex};

fn material_entries() -> Vec<BindingEntry> {
    vec![
        BindingEntry { name: "albedo", binding: 0, kind: BindingKind::texture_2d(ShaderStages::FRAGMENT) },
        BindingEntry { name: "albedo_sampler", binding: 1, kind: BindingKind::sampler(ShaderStages::FRAGMENT) },
    ]
}

let material = MaterialBuilder::new(SHADER)
    .label("lit")
    .vertex_layouts(vec![Vertex::layout()])
    .entries(vec![GroupEntry::Own(material_entries())])   // see Bind Groups and Layouts
    .targets(vec![ColorTargetState {
        format: backend.surface_format(),
        blend: None,
        write_mask: Default::default(),
    }])
    .build_asset("lit", &mut materials);
```

Everything left unset defaults to: `vertex_entry`/`fragment_entry` (`"vs_main"`/`"fs_main"`), `cull_mode: Some(Face::Back)`, no depth testing, fill polygon mode, `sample_count: 1` (see [MSAA](./msaa.md) for when that needs to change), no entries at all — `.entries(...)` is entirely optional, for a shader that doesn't declare any `@group` of its own.

## A material instance (concrete resources bound to a material)

[`MaterialInstance`](../src/wgpu/instance.rs) is plain data too — build one via [`MaterialInstanceBuilder`](../src/wgpu/instance.rs): chain one binding method per named entry, matched against the material's own `entries` by name:

```rust
use pebble::wgpu::instance::MaterialInstanceBuilder;
use pebble::wgpu::samplers::SamplerKind;

let instance = MaterialInstanceBuilder::new(material)   // material: Handle<Material>
    .texture("albedo", brick)                    // brick: Handle<Texture>
    .sampler("albedo_sampler", SamplerKind::LinearRepeat)
    .build_asset("brick_instance", &mut instances);
```

Each binding call's `name` is matched against the material's own `BindingEntry::name`s to find the right `@binding(N)` — the names must match the ones the material's `entries` declared, or the instance fails to upload. `.texture`/`.texture_array`/`.cubemap` take the *source* type's typed `Handle<T>` (`Handle<Texture>`, not a raw id) — resolved against `ProcessedAssets<GPUTexture>` once that texture itself has finished uploading, the same dependency-waiting behavior as any other `Deps`.

`.uniform(name, bytes)`/`.storage(name, bytes)` allocate and own a buffer themselves, updatable later via `instance.update(name, &bytes)`, or read back to the CPU via `instance.buffer(name).read()`/`read_as::<T>()` — see [Compute Pipelines](./compute-pipelines.md#reading-the-result-back). `.param(name, BindingInstanceEntry)` is the escape hatch for a dynamically-selected entry kind (building bindings in a loop over heterogeneous data, say) — prefer the typed methods above when the kind is known statically.

`SamplerKind::LinearRepeat` pulls from a small global cache of common sampler configurations ([`GlobalSamplers`](./samplers.md), set up automatically by `WGPUPlugin`) rather than creating a new sampler per instance — samplers are cheap to share and there's rarely a reason not to.

## Rendering with a material and instance

```rust
fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    instances: Res<ProcessedAssets<GPUMaterialInstance>>,
    mut query: Query<(&Handle<Mesh>, &Handle<MaterialInstance>)>,
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

Two things worth noticing:

- **`Res<ProcessedAssets<GPUMesh>>`, not `Assets<Mesh>`.** `render` reads the *uploaded* GPU-side objects (see [The Asset Pipeline and Handles](./the-asset-pipeline.md)), not the CPU-side descriptors that produced them.
- **`meshes.get`/`instances.get`/`materials.get` return `Option`, and a miss just `continue`s.** An asset might genuinely not be uploaded yet on the very first few frames — same "not ready yet, not an error" shape as every other async boundary in this book. The query looks up the material *through* the instance (`instance.target`, a `RawAssetHandle`) rather than holding a separate material handle on the entity — the entity only needs to know its mesh and its instance; the instance already knows which material it belongs to. `instance.target` is one of the few places `RawAssetHandle` shows up directly (see [The Asset Pipeline and Handles](./the-asset-pipeline.md#handlet-a-typed-reference)) — `materials.get` needs it to cross from the *instance's* `ProcessedAssets<GPUMaterialInstance>` into the *material's* `ProcessedAssets<GPUMaterial>`, two different `T`s no single typed `Handle<T>` could refer to at once.

See [Recording a Render Pass](./rendering-pass-recording.md) for the full set of draw-time methods.
