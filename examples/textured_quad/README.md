# Example: Textured Quad

Draws a textured quad by introducing a third asset type (`GPUTexture`) and a **dependent asset** (`GPUMaterialInstance`) whose upload requires two already-processed assets to be ready first.

Builds on the concepts from [hello_triangle](../hello_triangle/README.md) — read that first.

---

## What you will learn

- How to upload image data to a GPU texture
- How to express asset-to-asset dependencies using `type Deps<'a>`
- How `upload` returning `None` re-queues a handle until dependencies are ready
- How bind groups connect textures to pipelines

---

## Step 1 — Add a texture asset type

```rust
struct Texture { path: &'static str }

struct GPUTexture {
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    view:    wgpu::TextureView,
}

impl Asset<WGPUBackend> for GPUTexture {
    type Source = Texture;
    type Deps<'a> = ();   // only needs the backend

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, _deps: &Self::Deps<'a>) -> Option<Self> {
        let img  = image::open(source.path).expect("failed to load image");
        let rgba = img.to_rgba8();
        // create wgpu texture, write pixel data via queue, create view + sampler
        Some(Self { texture, sampler, view })
    }
}
```

Nothing special here — same pattern as `GPUMesh`. The `wgpu::Queue::write_texture` call copies the CPU pixel data into the GPU texture on upload.

---

## Step 2 — Split material into base + instance

Rather than a single "material with texture baked in", this example separates:

- **`Material` / `GPUMaterial`** — compiled shader pipeline and bind group layout. Does not know about a specific texture.
- **`MaterialInstance` / `GPUMaterialInstance`** — a concrete bind group that wires a specific texture into the material's layout.

This lets you reuse one pipeline with many different textures.

```rust
struct MaterialInstance {
    base:      RawAssetHandle,   // handle to GPUMaterial
    albedo_id: RawAssetHandle,   // handle to GPUTexture
}
```

---

## Step 3 — Express the dependency in `Deps`

`GPUMaterialInstance` cannot be created until `GPUMaterial` and `GPUTexture` are both uploaded. Declare that using `Deps`:

```rust
impl Asset<WGPUBackend> for GPUMaterialInstance {
    type Source = MaterialInstance;
    type Deps<'a> = (
        Res<'a, ProcessedAssets<GPUMaterial>>,
        Res<'a, ProcessedAssets<GPUTexture>>,
    );

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'a>) -> Option<Self> {
        let (materials, textures) = deps;

        let base_mat   = materials.get(source.base)?;       // returns None if not ready yet
        let albedo_tex = textures.get(source.albedo_id)?;   // same

        // create bind group combining the pipeline layout with the texture view
        Some(Self { pipeline: base_mat.pipeline.clone(), bind_group })
    }
}
```

The `?` operator on `materials.get(…)` returns `None` from `upload`, which causes `AssetPlugin` to re-queue this handle and retry on the next tick. This is the intended way to wait for dependent assets — no manual ordering or callbacks required.

The `AssetSync` stage will keep retrying `GPUMaterialInstance` each tick until both `GPUMaterial` and `GPUTexture` are in `ProcessedAssets`, then it will succeed on the first tick where both are present.

---

## Step 4 — Register all four asset plugins

```rust
.add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new())
.add_plugin(AssetPlugin::<WGPUBackend, GPUMaterial>::new())
.add_plugin(AssetPlugin::<WGPUBackend, GPUTexture>::new())
.add_plugin(AssetPlugin::<WGPUBackend, GPUMaterialInstance>::new())
```

Plugin registration order does not matter. Each plugin's sync system independently checks whether its own `Deps` resources exist before processing.

---

## Step 5 — Populate assets and spawn the entity

```rust
fn setup(…) {
    let quad_mesh     = meshes.insert("quad", Mesh { … });
    let quad_mat      = materials.insert("quad", Material { vertex: …, fragment: … });
    let brick_texture = textures.insert("brick", Texture { path: "../assets/textures/brick.png" });

    // MaterialInstance references the raw handles for the base material and texture
    let quad_mat_inst = material_instances.insert("quad_brick", MaterialInstance {
        base:      quad_mat,
        albedo_id: brick_texture,
    });

    commands.spawn((
        Handle::<Mesh>::new(quad_mesh),
        Handle::<MaterialInstance>::new(quad_mat_inst),
    ));
}
```

The asset pipeline will process them in the right order automatically:
1. `GPUMesh`, `GPUMaterial`, `GPUTexture` upload as soon as the backend is ready.
2. `GPUMaterialInstance` keeps returning `None` until steps 1 are done, then uploads on the first tick where both are available.

---

## Step 6 — Render

```rust
fn render(…) {
    if let Some(mut pass) = frame.render_context([0.2, 0.3, 0.3, 1.0]) {
        for (mesh_id, mat_id) in query.iter() {
            let Some(mesh) = meshes.get(mesh_id.id) else { return };
            let Some(inst) = mat_inst.get(mat_id.id) else { return };

            pass.set_pipeline(&inst.pipeline);
            pass.set_bind_group(0, Some(&inst.bind_group), &[]);  // binds the texture
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
```

`inst.bind_group` was created from the material's layout with the texture's view and sampler already bound, so `set_bind_group` is the only extra call compared to `hello_triangle`.

---

## Full system stage order for this example

```
Startup      → setup: insert CPU assets, spawn entity
               GraphicsPlugin: kick off backend init
AssetSync    → GPUMesh, GPUMaterial, GPUTexture: upload (once backend is ready)
               GPUMaterialInstance: retry each tick until base + texture are ready
PreRender    → GraphicsPlugin: poll backend / resize
               RenderPlugin:   begin_frame
Render       → render: draw the textured quad
PostRender   → RenderPlugin:   end_frame
```
