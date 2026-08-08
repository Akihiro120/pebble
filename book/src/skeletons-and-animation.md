# Skeletons and Animation Clips

[`Skeleton`](../src/wgpu/skeleton.rs) and [`AnimationClip`](../src/wgpu/animation.rs) are plain CPU data — no [`Asset`](./the-asset-pipeline.md)/`Handle`/GPU upload involved, unlike everything else in the rendering chapters. A skeleton is a joint hierarchy; sampling a clip and turning the result into a matrix palette is ordinary interpolation and matrix math, nothing GPU-specific about it until the moment *you* write the result into a buffer of your own. There's no built-in way to bind a `Skeleton` to a shader, because there's nothing to bind — see [Loading glTF Models](./loading-gltf-models.md#putting-it-together) for the buffer/shader side of this.

## `Transform`: why not just `glam::Mat4`?

A joint's pose is [`Transform`](../src/wgpu/skeleton.rs) — separate translation/rotation/scale, not a single matrix:

```rust
pub struct Transform {
    pub translation: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}
```

Interpolating a `Mat4` directly (lerping its columns) is mathematically wrong — rotation has to slerp, not lerp component-wise. Keeping T/R/S apart lets [`AnimationClip::sample`](#animationclip) interpolate each correctly, composing them into a matrix only at the very end via `Transform::to_matrix`. `Transform::lerp(&self, other, t)` blends two poses the same way (translation/scale lerp, rotation slerp) — the building block for crossfading between two animations, covered below.

## `Skeleton`

A joint hierarchy: each [`Joint`](../src/wgpu/skeleton.rs) has a name, an optional parent (an index into the same skeleton's joint list — `None` for a root), an inverse bind matrix, and its own bind-pose `Transform`:

```rust
use pebble::wgpu::skeleton::{Joint, Skeleton, Transform};

let skeleton = Skeleton::new(vec![
    Joint { name: "root".into(), parent: None, inverse_bind_matrix: glam::Mat4::IDENTITY, local_bind_transform: Transform::IDENTITY },
    Joint { name: "arm".into(), parent: Some(0), inverse_bind_matrix: /* ... */, local_bind_transform: /* ... */ },
]);
```

`Skeleton::new` doesn't require `joints` to already be in parent-before-child order — a glTF file's own node array isn't guaranteed to be sorted that way — it computes and caches a valid traversal order internally via a topological sort, and panics if the graph has a cycle or an out-of-range parent index (a malformed skeleton, not a "not ready yet" condition worth tolerating).

Given a `Vec<Transform>` (one local pose per joint, typically from [`AnimationClip::sample`](#animationclip)):

- **`skeleton.world_matrices(&poses)`** walks the hierarchy and returns each joint's world-space matrix.
- **`skeleton.skinning_matrices(&poses)`** goes one step further — each joint's world matrix times its own `inverse_bind_matrix` — the actual matrix palette a shader needs, transforming a vertex straight from mesh-bind space into the current pose.

## `AnimationClip`

A name, a duration, and per-joint keyframe tracks (`JointTrack`) for translation/rotation/scale — any of the three may be empty, since glTF allows animating only some of a joint's properties:

```rust
let poses: Vec<Transform> = clip.sample(time, &skeleton);
let matrices = skeleton.skinning_matrices(&poses);
```

`sample` **clamps** `time` to `[0, duration]` — it does not loop. To loop, wrap the time yourself before calling it: `time.rem_euclid(clip.duration)`. Any joint (or T/R/S component) the clip doesn't animate falls back to that joint's own bind-pose `Transform`.

Only `Linear` and `Step` interpolation are supported — `CubicSpline` isn't representable by `Interpolation` at all; [`load_gltf`](./loading-gltf-models.md) returns an error rather than silently mishandling it (its accessors pack an in-tangent/value/out-tangent triple per keyframe, not a plain value).

## Driving animation yourself

Nothing here runs itself — there's no built-in "animation player" system, the same way there's no built-in camera. A typical setup is a small per-entity component and two systems you write:

```rust
struct AnimationState { skeleton: Arc<Skeleton>, clip: Arc<AnimationClip>, time: f32 }

fn advance_animation(time: Res<Time>, mut query: Query<&mut AnimationState>) {
    for state in query.iter() {
        state.time = (state.time + time.delta_seconds()).rem_euclid(state.clip.duration);
    }
}

fn update_joint_buffers(query: Query<(&AnimationState, &Handle<MaterialInstance>)>, instances: Res<ProcessedAssets<GPUMaterialInstance>>) {
    for (state, handle) in query.iter() {
        let Some(instance) = instances.get(handle.id) else { continue };
        let poses = state.clip.sample(state.time, &state.skeleton);
        let matrices = state.skeleton.skinning_matrices(&poses);
        instance.update("joint_matrices", bytemuck::cast_slice(&matrices));
    }
}
```

`GPUBindingInstance::update` (see [Materials](./materials.md#a-material-instance-concrete-resources-bound-to-a-material)) rewrites a storage buffer this instance already owns — the same primitive any other per-frame-updated instance buffer uses, nothing skinning-specific about it. `examples/skeletal_animation` runs exactly this loop end to end, including the WGSL vertex shader that actually performs the skinning blend.

## Blending between animations

`Transform::lerp` is the one primitive provided for this — everything past it (crossfade timing, per-bone blend masks, additive blending, blend trees) is left to you, since the right strategy depends entirely on what you're building:

```rust
let poses_a = clip_a.sample(time_a, &skeleton);
let poses_b = clip_b.sample(time_b, &skeleton);
let blended: Vec<Transform> = poses_a.iter().zip(&poses_b).map(|(a, b)| a.lerp(b, blend_t)).collect();
let matrices = skeleton.skinning_matrices(&blended);
```
