# Loading glTF Models

[`load_gltf`](../src/wgpu/gltf_loader.rs) parses geometry, a skeleton, and animation clips out of a `.gltf`/`.glb` file:

```rust
use pebble::wgpu::gltf_loader::load_gltf;

let model = load_gltf("assets/models/character.glb")?;
```

## Why `Result`, not `Option`/silent retry

Everything else fallible in the [asset pipeline](./the-asset-pipeline.md) — a missing backend, a texture whose file doesn't exist, an unregistered `GroupEntry::Global` name — returns `None` from `Asset::upload`, which the sync system silently retries every tick forever. That convention is right for genuinely transient conditions ("the backend isn't ready *yet*"), and wrong for a condition that can never resolve on its own: a `.gltf` path that's simply wrong, or a file that uses a glTF feature this loader doesn't support, will *never* start working no matter how many times it's retried.

`load_gltf` isn't part of that pipeline at all — it's a plain, synchronous function you call directly (in `main()`, a `.once()` system, wherever), and it returns a real `Result<LoadedModel, ModelLoadError>` you handle like any other fallible I/O:

```rust
let model = match load_gltf(path) {
    Ok(model) => model,
    Err(e) => {
        eprintln!("failed to load model: {e}");
        std::process::exit(1);
    }
};
```

## `LoadedModel`

```rust
pub struct LoadedModel {
    pub skinned_meshes: Vec<(String, SkinnedMesh)>,
    pub static_meshes: Vec<(String, Mesh)>,
    pub skeleton: Option<Skeleton>,
    pub animations: Vec<AnimationClip>,
}
```

`skinned_meshes` are primitives bound to the file's skin, as [`SkinnedMesh`](./skeletal-meshes.md); `static_meshes` are everything else in the same file (rigid props, environment pieces) as the ordinary [`Mesh`](./meshes.md) — not padded with identity joint weights just to force them through the skinned path. Both are plain, already-built values — `load_gltf` has no `&mut Assets<T>` to insert into (it isn't a system), so you insert them yourself:

```rust
for (name, mesh) in model.skinned_meshes {
    let handle = skinned_meshes.insert(&name, mesh); // ResMut<Assets<SkinnedMesh>>
}
```

`skeleton`/`animations` are the plain CPU data covered in [Skeletons and Animation Clips](./skeletons-and-animation.md) — `None`/empty if the file has no skin.

## Why not `Asset<B>`?

`Skeleton` and `AnimationClip` are pure CPU computation right up until you write a matrix palette into a buffer of your own — see [Skeletons and Animation Clips](./skeletons-and-animation.md) for the full rationale. Only the mesh *geometry* `load_gltf` extracts is a genuine GPU resource, going through the ordinary `SkinnedMesh`/`Mesh` asset pipeline like anything else.

## Scope

`load_gltf` covers geometry, a skeleton, and animation — nothing else. It never reads `document.materials()` or any embedded image data, even when the file references them: load your own textures via [`TextureBuilder`](./textures.md) and write your own [`Material`](./materials.md)/shader entirely separately, the same as every other example in this book.

A few glTF features are explicitly unsupported, returning `ModelLoadError::UnsupportedFeature` rather than a wrong or silently-degraded result:

- **More than one skin per file.** Exactly zero or one is handled.
- **`CUBICSPLINE` animation interpolation.** Only `LINEAR`/`STEP` — cubic-spline accessors pack an in-tangent/value/out-tangent triple per keyframe, not a plain value, and misreading one as a plain value would silently produce garbage rather than fail loudly.
- **Sparse accessors.**
- **Non-indexed primitives** — both `SkinnedMesh` and `Mesh` are index-buffer-only.
- **Morph targets.**

One more limitation, not an error: a joint whose real parent (in the glTF scene graph) isn't itself one of the skin's joints — an "Armature" root object that isn't a joint, say — is treated as a [`Skeleton`](./skeletons-and-animation.md) root, discarding that ancestor's transform. Most standard exports (a plain identity-transform armature root) aren't affected; a rig where that ancestor has a real, non-identity transform will render offset from where it should be.

`ModelLoadError` also covers ordinary I/O/parse failures (`Io`, `Parse`) and missing required data (`MissingData` — e.g. a skinned primitive with no `JOINTS_0`/`WEIGHTS_0`).

## Putting it together

`examples/skeletal_animation` is the full loop, end to end: `load_gltf` → insert the returned `SkinnedMesh` into `Assets<SkinnedMesh>` → build a `Material`/`MaterialInstance` with a `joint_matrices` storage-buffer entry (`BindingKind::storage_buffer_read_only`) → a `Res<Time>`-driven system sampling the clip and calling `skeleton.skinning_matrices(...)` every frame → `GPUBindingInstance::update("joint_matrices", ...)` → a hand-written WGSL vertex shader doing the actual weighted joint-matrix blend. Nothing about rendering a skinned mesh is automatic — `load_gltf` and `Skeleton`/`AnimationClip` hand you real data; the shader, the pipeline, and the draw call are yours, same as everything else in `pebble::wgpu`.
