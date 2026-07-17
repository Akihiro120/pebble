# Example: Hello Triangle

Draws a single coloured triangle using the asset pipeline. Introduces the `Asset` trait, `AssetPlugin`, typed `Handle`s, and ECS entities that tie mesh and material together.

Builds on the concepts from [clear_screen](../clear_screen/README.md) — read that first.

---

## What you will learn

- How to define CPU-side source assets (`Mesh`, `Material`)
- How to implement `Asset<B>` to upload them to the GPU
- How `AssetPlugin` drives the CPU → GPU sync automatically
- How to use `Handle<T>` to reference assets from entities
- How to query entities and look up processed assets at render time

---

## Step 1 — Define source and GPU asset types

Every asset type comes in two halves:

```rust
// CPU side — stored in Assets<Mesh>
struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

// GPU side — stored in ProcessedAssets<GPUMesh>
struct GPUMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}
```

The CPU type is what you fill in from game code. The GPU type is what the renderer actually uses.

---

## Step 2 — Implement `Asset<B>` for each GPU type

```rust
impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, _deps: &Self::Deps<'a>) -> Option<Self> {
        let vertex_buffer = backend.device.create_buffer_init(…);
        let index_buffer  = backend.device.create_buffer_init(…);
        Some(Self { vertex_buffer, index_buffer, index_count })
    }
}
```

- `type Source` — the CPU type to read from.
- `type Deps<'a> = ()` — no extra resources needed beyond the backend. If you needed another processed asset (e.g. a texture for a material instance) you would list it here.
- `upload` — the conversion. Return `Some(gpu_asset)` on success, or `None` to re-queue for the next tick (useful when a dependency isn't ready yet).

The same pattern applies to `GPUMaterial`, which compiles SPIR-V shaders and creates a `wgpu::RenderPipeline`.

---

## Step 3 — Register the asset plugins

```rust
.add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new())
.add_plugin(AssetPlugin::<WGPUBackend, GPUMaterial>::new())
```

Each `AssetPlugin<B, T>` call:
1. Inserts `Assets<T::Source>` — the CPU-side storage.
2. Inserts `ProcessedAssets<T>` — the GPU-side storage.
3. Adds a system on `AssetSync` that drains the dirty queue from `Assets<T::Source>` and calls `T::upload` for each pending entry, populating `ProcessedAssets<T>`.

The sync system silently waits until both `WGPUBackend` and all of `T`'s `Deps` are available as resources, so ordering is automatic.

---

## Step 4 — Populate assets and spawn entities in Startup

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
) {
    let triangle_mesh = meshes.insert("triangle", Mesh { … });
    let triangle_mat  = materials.insert("triangle", Material { vertex: …, fragment: … });

    commands.spawn((
        Handle::<Mesh>::new(triangle_mesh),
        Handle::<Material>::new(triangle_mat),
    ));
}
```

`assets.insert(name, value)` stores the CPU asset and pushes the handle onto the **dirty queue**. On the next `AssetSync` tick the backend will upload it.

`Handle<T>` is a typed, `Copy` wrapper around a `RawAssetHandle`. Attaching it to an entity is how you associate a drawable object with its assets without duplicating data.

---

## Step 5 — Render by querying entities

```rust
fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    mut query: Query<(&Handle<Mesh>, &Handle<Material>)>,
) {
    if let Some(mut pass) = frame.render_context([0.2, 0.3, 0.3, 1.0]) {
        for (mesh_id, mat_id) in query.iter() {
            let Some(mesh) = meshes.get(mesh_id.id) else { return };
            let Some(mat)  = materials.get(mat_id.id) else { return };

            pass.set_pipeline(&mat.pipeline);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
```

The `Query` iterates every entity that has both a `Handle<Mesh>` and a `Handle<Material>`. For each, we look up the corresponding GPU asset. The `let Some(…) else { return }` guards handle the frames where assets haven't finished uploading yet — they simply skip that draw call.

---

## Full system stage order for this example

```
Startup      → setup: insert CPU assets, spawn entity with handles
               GraphicsPlugin: kick off backend init
AssetSync    → AssetPlugin<GPUMesh>:     upload pending meshes
               AssetPlugin<GPUMaterial>: compile pending pipelines
PreRender    → GraphicsPlugin: poll backend / resize
               RenderPlugin:   begin_frame
Render       → render: draw the triangle
PostRender   → RenderPlugin:   end_frame
```
