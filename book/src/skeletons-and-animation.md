# Skeletons and Animation

## Data types: `Skeleton`, `AnimationClip`, `Transform`

[`Skeleton`](../src/wgpu/skeleton.rs) and [`AnimationClip`](../src/wgpu/animation.rs) are plain CPU data — no [`Asset`](./the-asset-pipeline.md)/`Handle`/GPU upload involved, unlike everything else in the rendering chapters. A skeleton is a joint hierarchy; sampling a clip and turning the result into a matrix palette is ordinary interpolation and matrix math.

### `Transform`: why not just `glam::Mat4`?

A joint's pose is [`Transform`](../src/wgpu/skeleton.rs) — separate translation/rotation/scale, not a single matrix:

```rust
pub struct Transform {
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}
```

Interpolating a `Mat4` directly (lerping its columns) is mathematically wrong — rotation has to slerp, not lerp component-wise. Keeping T/R/S apart lets [`AnimationClip::sample`](#animationclip) interpolate each correctly, composing them into a matrix only at the very end. `Transform::lerp(&self, other, t)` blends two poses the same way (translation/scale lerp, rotation slerp).

### `Skeleton`

A joint hierarchy: each [`Joint`](../src/wgpu/skeleton.rs) has a name, an optional parent (an index into the same skeleton's joint list — `None` for a root), an inverse bind matrix, and its own bind-pose `Transform`:

```rust
use pebble::wgpu::skeleton::{Joint, Skeleton, Transform};

let skeleton = Skeleton::new(vec![
    Joint { name: "root".into(), parent: None, inverse_bind_matrix: glam::Mat4::IDENTITY, local_bind_transform: Transform::IDENTITY },
    Joint { name: "arm".into(), parent: Some(0), inverse_bind_matrix: /* ... */, local_bind_transform: /* ... */ },
]);
```

Given a `Vec<Transform>` (one local pose per joint):

- **`skeleton.world_matrices(&poses)`** — each joint's world-space matrix.
- **`skeleton.skinning_matrices(&poses)`** — world matrix times inverse bind matrix, the actual palette a shader needs.

### `AnimationClip`

A name, a duration, and per-joint keyframe tracks. `sample` **clamps** `time` to `[0, duration]` — looping is handled by `AnimationPlayer` (see below). Only `Linear` and `Step` interpolation are supported — `CubicSpline` returns an error.

## `AnimationPlayer`: built-in playback

[`AnimationPlayer`](../src/wgpu/player.rs) is an ECS component that manages animation playback for a skinned entity. Attach it alongside a `Handle<SkinnedMesh>` and a `Handle<Material>`:

```rust
commands.spawn((mesh_handle, material_handle, loaded.player));
```

### Controlling playback

```rust
player.play("walk");                  // hard cut to clip, loop
player.play_once("attack");           // play once, hold last frame
player.crossfade("run", 0.3);         // blend from current → "run" over 0.3 s
player.set_speed(2.0);                // double speed
player.pause();  player.resume();
player.set_time(1.5);                 // jump to specific time
```

`player.clip_names()` returns an iterator over all loaded clip names — useful for playing the first available clip without knowing its name in advance:

```rust
if let Some(name) = loaded.player.clip_names().next().map(|s| s.to_string()) {
    loaded.player.play(&name);
}
```

### Advancing time

`SkinnedBatchingPlugin` reads matrices but does **not** advance time — add a system that calls `advance` each tick:

```rust
fn advance_animation(time: Res<Time>, mut query: Query<&mut AnimationPlayer>) {
    for player in query.iter() {
        player.advance(time.delta_seconds());
    }
}
```

### IK and procedural animation via `Pose`

`compute_pose()` samples the current clip into a [`Pose`](../src/wgpu/player.rs) you can modify before it becomes matrices — useful for IK or procedural overrides:

```rust
let mut pose = player.compute_pose();
// two-bone IK: you supply the math, Pose handles local/world conversion
pose.set_world_rotation(upper_leg, rotation_upper);
pose.set_world_rotation(lower_leg, rotation_lower);
player.set_matrices(pose.skinning_matrices()); // persists until clear_matrices()
```

`player.clear_matrices()` removes the override and returns to sampling the current clip.

## `SkinnedBatchingPlugin`: GPU-batched rendering

[`SkinnedBatchingPlugin`](../src/wgpu/skinning.rs) collects all entities with `(Handle<Material>, Handle<SkinnedMesh>, AnimationPlayer)` components each `PreRender` tick, writes their joint matrices into a per-`(material, mesh)` GPU storage buffer, and stores the draw batch metadata in [`SkinnedBatchStorage`](../src/wgpu/skinning.rs):

```rust
App::new()
    // ...
    .add_plugin(SkinnedBatchingPlugin)
    // ...
```

The WGSL skinning bind group (`"pebble_skinning"`) is registered automatically in `GlobalLayoutPool`. In a material that uses it, declare `GroupEntry::Global("pebble_skinning")` at whichever group index the shader expects:

```rust
MaterialBuilder::new(SHADER)
    .with_vertex_layouts(vec![SkinnedVertex::layout()])
    .with_entries(vec![GroupEntry::Global("pebble_skinning")])
    // ...
    .build_asset("skinned", &mut materials);
```

The WGSL layout the plugin fills:

```wgsl
struct SkinningInfo { joint_count: u32 }
@group(N) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;
@group(N) @binding(1) var<uniform>        skin_info:     SkinningInfo;

// in vs_main — use instance_index to offset into the flat matrix array:
let base = instance_index * skin_info.joint_count;
let skin =
    joint_matrices[base + joint_indices.x] * joint_weights.x +
    joint_matrices[base + joint_indices.y] * joint_weights.y +
    joint_matrices[base + joint_indices.z] * joint_weights.z +
    joint_matrices[base + joint_indices.w] * joint_weights.w;
```

`instance_index * joint_count` gives the per-entity base offset into the flat buffer — instanced `draw_indexed(0..index_count, 0, 0..instance_count)` calls let one draw cover all entities sharing the same `(material, mesh)` pair.

### The render loop

```rust
fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<Assets<Material>>,
    meshes: Res<Assets<SkinnedMesh>>,
    renderer: Option<Res<SkinnedBatchRenderer>>,
    storage: Option<Res<SkinnedBatchStorage>>,
) {
    let Some(renderer) = renderer else { return };
    let Some(storage) = storage else { return };
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for batch in storage.batches.iter() {
        let Some(mesh) = meshes.get(Handle::<SkinnedMesh>::new(batch.mesh)) else { continue };
        let Some(material) = materials.get(Handle::<Material>::new(batch.material)) else { continue };
        let Some(bind_group) = renderer.bind_group(batch.material, batch.mesh) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..batch.instance_count);
    }
}
```

`SkinnedBatchRenderer` and `SkinnedBatchStorage` are `Option<Res<...>>` above because they're inserted by `SkinnedBatchingPlugin`'s startup system — they don't exist on the very first frame before that runs.

## Loading from glTF with `SkinnedModelBuilder`

The recommended way to load a glTF file with a skinned mesh and animation clips into a ready-to-use `AnimationPlayer`:

```rust
fn setup(
    mut commands: Commands,
    mut skinned_meshes: ResMut<Assets<SkinnedMesh>>,
    mut materials: ResMut<Assets<Material>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let mut loaded = SkinnedMeshBuilder::from_file("assets/character.gltf")
        .with_animation("run", "assets/character_run.gltf") // extra clips, optional
        .build(&mut skinned_meshes)
        .ok()?;

    let mesh_handle = loaded.mesh()?;

    // Play first available clip (name comes from the glTF file)
    if let Some(name) = loaded.player.clip_names().next().map(|s| s.to_string()) {
        loaded.player.play(&name);
    }

    let material = MaterialBuilder::new(SHADER)
        .with_vertex_layouts(vec![SkinnedVertex::layout()])
        .with_entries(vec![GroupEntry::Global("pebble_skinning")])
        // ...
        .build_asset("skinned", &mut materials);

    commands.spawn((mesh_handle, material, loaded.player));
    Some(())
}
```

`SkinnedModelBuilder::build` is synchronous — it calls `load_gltf` internally and returns `Result<LoadedSkinnedMesh, ModelLoadError>`. Using `.ok()?` inside a `-> Option<()>` once-system means a load failure silently retries, which is usually wrong for a path error; handle the `Result` explicitly if you need a clear error:

```rust
let loaded = SkinnedMeshBuilder::from_file(path)
    .build(&mut skinned_meshes)
    .expect("failed to load character");
```

## Blending between animations

`Transform::lerp` is the one primitive provided for manual blending — everything past it (custom blend trees, additive blending, per-bone masks) is left to you via `compute_pose()` + `set_matrices()`:

```rust
let mut pose_a = player_a.compute_pose();
let mut pose_b = player_b.compute_pose();
// blend per-joint and override — player.set_matrices() sends the result to the GPU
```

For straightforward crossfades between two clips on the same entity, prefer `player.crossfade(name, duration)` — it handles the interpolation internally.
