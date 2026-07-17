# Example: Orbit Camera

Renders a textured cube with a perspective camera that orbits around the origin over time. Introduces custom `Plugin` types, a `Time` resource, a depth buffer, and camera uniform buffers — all built from first principles using the framework's plugin and system APIs.

Builds on the concepts from [textured_quad](../textured_quad/README.md) — read that first.

---

## What you will learn

- How to write your own `Plugin` structs
- How to manage a GPU uniform buffer as a resource (not an asset)
- How to use `type Deps<'a>` to wait for resources created at runtime
- How to create and use a depth texture
- How to use `Time` as a resource for animation
- How to use `begin_pass` directly with colour and depth attachments

---

## Step 1 — The `TimePlugin`

Time tracking is a self-contained plugin:

```rust
struct TimePlugin;
impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Time {
            time:       Instant::now(),
            last_time:  Instant::now(),
            delta_time: 0.0,
        })
        .add_system(SystemStage::PreUpdate, update_delta_time);
    }
}
```

`build` receives `&mut App` and can add resources, systems, and more plugins. This is the standard Pebble extension point.

The `update_delta_time` system runs every `PreUpdate`, computing the elapsed seconds since the last frame and storing it in `time.delta_time`. Other systems can then borrow `Res<Time>` to read `time.time.elapsed()` for absolute time or `time.delta_time` for frame-relative values.

---

## Step 2 — The `DepthPlugin`

A depth texture is required for 3D rendering so closer geometry correctly occludes further geometry.

```rust
struct DepthPlugin;
impl Plugin for DepthPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(SystemStage::PreRender, create_depth);
    }
}
```

`create_depth` checks whether both `WGPUBackend` and `DepthTexture` are present. If the backend exists but the depth texture does not, it creates one and inserts it:

```rust
fn create_depth(mut commands: Commands, backend: Option<Res<WGPUBackend>>, depth: Option<Res<DepthTexture>>) {
    if let Some(backend) = backend && let None = depth {
        let texture = backend.device.create_texture(…Depth16Unorm…);
        commands.insert_resource(DepthTexture { texture, view });
    }
}
```

Using `Option<Res<…>>` is the idiomatic way to make a system wait for a resource that is created asynchronously (here: the backend must exist first). `Commands::insert_resource` defers the insertion to after the current stage finishes.

---

## Step 3 — The `CameraPlugin`

The camera needs a GPU uniform buffer (a `wgpu::Buffer` holding the view/projection matrices) and a bind group. These cannot be created until the backend is ready, so they are created lazily in a system rather than at startup.

```rust
struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(SystemStage::Update, (create_camera, update_camera, update_camera_buffer));
    }
}
```

Three systems are registered as a tuple using `add_systems`, which registers them all at the same stage in one call.

**`create_camera`** — waits for `WGPUBackend`, then creates the uniform buffer, bind group layout, and bind group, inserting the result as a `Camera` resource:

```rust
fn create_camera(mut commands: Commands, backend: Option<Res<WGPUBackend>>, camera: Option<Res<Camera>>) {
    if let None = camera && let Some(backend) = backend {
        // create uniform buffer + bind group layout + bind group
        commands.insert_resource(Camera { bind_group_layout, bind_group, buffer });
    }
}
```

**`update_camera`** — queries the `CameraComponent` entity and writes the orbiting view matrix into the `CameraUniform`:

```rust
let radius = 5.0;
let cam_x = f32::sin(elapsed) * radius;
let cam_y = f32::sin(elapsed) * radius;
let cam_z = f32::cos(elapsed) * radius;
active_camera.uniform.view = glam::Mat4::look_at_rh(
    glam::Vec3::new(cam_x, cam_y, cam_z),
    glam::Vec3::default(),  // look at the origin
    glam::Vec3::Y,
);
```

**`update_camera_buffer`** — copies the updated `CameraUniform` into the GPU buffer via `queue.write_buffer`. This is done every frame so the shader always sees the latest matrices.

---

## Step 4 — Asset dependency on `Camera` and `DepthTexture`

The `GPUMaterial` for this example needs to know the camera bind group layout (to include it in the pipeline layout) and the depth format (to configure depth testing). Both are resources that may not exist when the first `AssetSync` fires.

This is handled via `Deps`:

```rust
impl Asset<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = (Res<'a, Camera>, Res<'a, DepthTexture>);

    fn upload<'a>(source: &Self::Source, backend: &WGPUBackend, deps: &Self::Deps<'a>) -> Option<Self> {
        let (camera, depth) = deps;

        // include camera.bind_group_layout in the pipeline layout
        // use depth.texture.format() for the depth_stencil state
        Some(Self { pipeline, bind_group_layout })
    }
}
```

`AssetPlugin` calls `Dependencies::try_gather` before calling `upload`. If either `Camera` or `DepthTexture` is missing from resources, `try_gather` returns `None`, the system logs a trace message, and retries next tick. Once both are present the pipeline is compiled with the correct layout and depth format.

---

## Step 5 — Spawning the scene in Startup

```rust
fn setup(mut commands: Commands, mut meshes: …, mut materials: …, mut textures: …, mut material_instances: …) {
    // Insert a full cube mesh (24 vertices, 6 faces, normals included)
    let cube_mesh = meshes.insert("quad", Mesh { vertices: […], indices: […] });

    // Insert the lit shader (reads camera bind group at slot 0)
    let lit_mat = materials.insert("lit", Material { vertex: …lit.vert.spv…, fragment: …lit.frag.spv… });

    // Insert the brick texture
    let brick_texture = textures.insert("brick", Texture { path: "../assets/textures/brick.png" });

    // MaterialInstance binds the texture into the material (waits for GPUMaterial + GPUTexture)
    let lit_mat_inst = material_instances.insert("quad_brick", MaterialInstance {
        base:      lit_mat,
        albedo_id: brick_texture,
    });

    // Spawn the camera entity (holds the uniform that update_camera writes to)
    commands.spawn((CameraComponent { uniform: CameraUniform::default() }, TransformComponent { … }));

    // Spawn the cube entity
    commands.spawn((Handle::<Mesh>::new(cube_mesh), Handle::<MaterialInstance>::new(lit_mat_inst)));
}
```

---

## Step 6 — Rendering with a depth pass

The render system uses `begin_pass` directly (rather than the `render_context` shortcut) so it can attach a depth buffer:

```rust
if let Some(mut pass) = frame.begin_pass(Pass {
    colors: &[ColorTarget::Default { clear: Some([0.2, 0.3, 0.3, 1.0]) }],
    depth:  Some(DepthTarget { attachment: &depth.view, clear: Some(1.0) }),
}) {
    pass.set_bind_group(0, Some(&camera.bind_group), &[]);   // camera matrices

    for (mesh_id, mat_id) in query.iter() {
        pass.set_pipeline(&inst.pipeline);
        pass.set_bind_group(1, Some(&inst.bind_group), &[]); // texture
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(…);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
```

`clear: Some(1.0)` clears the depth buffer to the maximum value (far plane) at the start of each frame. Fragments with a smaller depth (closer to the camera) pass the `Less` depth compare and write their colour.

Bind group 0 is the camera (shared across all draws). Bind group 1 is the material instance (per-draw, holds the texture).

---

## Full system stage order for this example

```
Startup      → setup: spawn camera entity, cube entity, insert CPU assets
               GraphicsPlugin: kick off backend init
PreUpdate    → TimePlugin: compute delta_time
Update       → CameraPlugin: create_camera (once, when backend is ready)
               CameraPlugin: update_camera (orbit matrices)
               CameraPlugin: update_camera_buffer (write to GPU)
AssetSync    → GPUMesh, GPUTexture: upload (needs backend only)
               GPUMaterial: upload (needs backend + Camera + DepthTexture)
               GPUMaterialInstance: upload (needs GPUMaterial + GPUTexture)
PreRender    → DepthPlugin: create_depth (once, when backend is ready)
               GraphicsPlugin: poll backend / resize
               RenderPlugin:   begin_frame
Render       → render: draw the orbiting cube
PostRender   → RenderPlugin:   end_frame
```
