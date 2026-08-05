# Materials

A `GPUMaterial` is a render pipeline — compiled once, describing *what shape* of bind group a shader expects (a texture at binding 0, a sampler at binding 1). It says nothing about *which* texture. That's deliberate: the same brick-wall material should be reusable for a floor and a crate without recompiling a pipeline for each — only the bound texture differs. A `GPUMaterialInstance` is that missing piece: a concrete bind group, built by resolving a material's declared entries against actual assets.

## Building a material

[`MaterialDescriptor`](../src/wgpu/material.rs) → `Assets<MaterialDescriptor>::insert` → `Handle<MaterialDescriptor>`, uploaded automatically to `ProcessedAssets<GPUMaterial>` (registered by `WGPUPlugin`, no manual plugin needed):

```rust
use pebble::wgpu::{binding::{BindingEntry, BindingKind}, material::MaterialDescriptor, mesh::Vertex};

fn material_entries() -> Vec<BindingEntry> {
    vec![
        BindingEntry { name: "albedo", binding: 0, kind: BindingKind::texture_2d(ShaderStages::FRAGMENT) },
        BindingEntry { name: "albedo_sampler", binding: 1, kind: BindingKind::sampler(ShaderStages::FRAGMENT) },
    ]
}

let material = materials.insert("lit", MaterialDescriptor {
    label: Some("lit"),
    shader_source: SHADER,
    vertex_layouts: vec![Vertex::layout()],
    entries: material_entries(),   // Vec<BindingEntry>, see Bind Groups and Layouts
    own_group: Some(0),            // None if entries is empty — no bind group at all
    targets: vec![ColorTargetState {
        format: backend.surface_format(),
        blend: None,
        write_mask: Default::default(),
    }],
    ..Default::default()
});
```

`..Default::default()` covers `vertex_entry`/`fragment_entry` (`"vs_main"`/`"fs_main"`), `cull_mode: Some(Face::Back)`, no depth testing, fill polygon mode, `sample_count: 1` (see [MSAA](./msaa.md) for when that needs to change), no `extra_layouts`. A material with no bind group at all (`entries: vec![]`, `own_group: None`) is valid too — for a shader that doesn't declare any `@group` of its own.

## A material instance (concrete resources bound to a material)

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

Each `(name, BindingInstanceEntry)` pair is matched against the material's own `BindingEntry::name`s to find the right `@binding(N)` — the names must match the ones the material's `entries` declared, or the instance fails to upload. `MaterialInstanceDescriptor::new` takes `RawAssetHandle`s, not typed `Handle<T>`s — that's `material.id`/`brick.id`, unwrapping the typed handles. This is the one place `RawAssetHandle` shows up directly (see [The Asset Pipeline and Handles](./the-asset-pipeline.md#handlet-a-typed-reference)): an instance crosses between the *material's* `ProcessedAssets<GPUMaterial>` and the *texture's* `ProcessedAssets<GPUTexture>`, two different `T`s that no single typed `Handle<T>` could refer to at once.

`BindingInstanceEntry` variants: `Texture`/`TextureArray`/`Cubemap`/`Sampler` (all by handle/kind, resolved against existing processed assets) and `Uniform(Vec<u8>)`/`Storage(Vec<u8>)` (a buffer the instance allocates and owns itself, updatable later via `instance.update(name, &bytes)`, or read back to the CPU via `instance.buffer(name).read()`/`read_as::<T>()` — see [Compute Pipelines](./compute-pipelines.md#reading-the-result-back)).

`SamplerKind::LinearRepeat` pulls from a small global cache of common sampler configurations ([`GlobalSamplers`](./samplers.md), set up automatically by `WGPUPlugin`) rather than creating a new sampler per instance — samplers are cheap to share and there's rarely a reason not to.

## Rendering with a material and instance

```rust
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

Two things worth noticing:

- **`Res<ProcessedAssets<GPUMesh>>`, not `Assets<MeshDescriptor>`.** `render` reads the *uploaded* GPU-side objects (see [The Asset Pipeline and Handles](./the-asset-pipeline.md)), not the CPU-side descriptors that produced them.
- **`meshes.get`/`instances.get`/`materials.get` return `Option`, and a miss just `continue`s.** An asset might genuinely not be uploaded yet on the very first few frames — same "not ready yet, not an error" shape as every other async boundary in this book. The query looks up the material *through* the instance (`instance.target`, a `RawAssetHandle`) rather than holding a separate material handle on the entity — the entity only needs to know its mesh and its instance; the instance already knows which material it belongs to.

See [Recording a Render Pass](./rendering-pass-recording.md) for the full set of draw-time methods.
