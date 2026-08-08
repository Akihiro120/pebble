# Skeletal Meshes

## `SkinnedVertex`

[`SkinnedVertex`](../src/wgpu/skinned_mesh.rs) is [`Vertex`](./meshes.md#the-built-in-vertex-layout)'s 4 fields (position, UV, normal, tangent) plus per-vertex skinning data — up to 4 joint indices and matching weights:

```rust
use pebble::wgpu::skinned_mesh::SkinnedVertex;

let v = SkinnedVertex::new(
    glam::Vec3::new(0.0, 0.6, 0.0),
    glam::Vec2::ZERO,
    glam::Vec3::Z,
    glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
    [0, 1, 0, 0],        // joint_indices — up to 4 joints this vertex is bound to
    [0.7, 0.3, 0.0, 0.0], // joint_weights — matching weights, usually summing to ~1.0
);
```

`joint_indices` is `[u16; 4]` — glTF's own `JOINTS_n` accessor is spec-limited to 8/16-bit indices, so `u16` already covers every legal glTF skeleton losslessly, at half the bytes of `u32`.

`SkinnedVertex::layout()` occupies vertex buffer locations 0–3 (same meaning as [`Vertex`](./meshes.md#the-built-in-vertex-layout)'s own 0–3) and 8–9 — deliberately skipping 4–7, which [`InstanceVertex`](./meshes.md#the-built-in-vertex-layout) uses, so a skinned mesh can still be paired with per-instance data in one pipeline without either layout changing. On the WGSL side, declare `@location(8) joint_indices: vec4<u32>` and `@location(9) joint_weights: vec4<f32>` — every integer vertex format widens to `vec4<u32>` in the shader regardless of the source width.

## Building a skinned mesh

[`SkinnedMesh`](../src/wgpu/skinned_mesh.rs) is plain data with no public constructors of its own — the only way to build one is [`SkinnedMeshBuilder`](../src/wgpu/skinned_mesh.rs), same shape as [`MeshBuilder`](./meshes.md#building-a-mesh):

```rust
use pebble::wgpu::skinned_mesh::SkinnedMeshBuilder;

let mesh = SkinnedMeshBuilder::new(vertices, indices).build_asset("character", &mut skinned_meshes);
```

`.build_asset` returns a `Handle<SkinnedMesh>`, uploading to `ProcessedAssets<GPUSkinnedMesh>` automatically through the same [asset pipeline](./the-asset-pipeline.md) as every other GPU resource — `GPUSkinnedMesh` exposes `vertex_buffer`/`index_buffer`/`index_count`, exactly like [`GPUMesh`](./meshes.md#building-a-mesh).

In practice you'll rarely hand-author `SkinnedVertex` data yourself — see [Loading glTF Models](./loading-gltf-models.md) for the normal way a `SkinnedMesh` gets built, and [Skeletons and Animation Clips](./skeletons-and-animation.md) for what pairs with it at draw time (a `Skeleton` to compute joint matrices, an `AnimationClip` to animate them).
