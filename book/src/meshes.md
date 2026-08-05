# Meshes and Vertices

## The built-in vertex layout

[`Vertex`](../src/wgpu/mesh.rs) is a fixed layout: position, texture coordinates, normal, and tangent — everything a lit, textured mesh needs. A shader that only reads position still needs a buffer with all four fields populated, since the buffer's layout is fixed regardless of what any one shader chooses to read:

```rust
use pebble::wgpu::mesh::Vertex;

fn triangle_vertices() -> Vec<Vertex> {
    let uv = glam::Vec2::ZERO;
    let normal = glam::Vec3::Z;
    let tangent = glam::Vec4::new(1.0, 0.0, 0.0, 1.0);
    vec![
        Vertex::new(glam::Vec3::new(0.0, 0.6, 0.0), uv, normal, tangent),
        Vertex::new(glam::Vec3::new(-0.6, -0.6, 0.0), uv, normal, tangent),
        Vertex::new(glam::Vec3::new(0.6, -0.6, 0.0), uv, normal, tangent),
    ]
}
```

`Vertex::layout()`/`InstanceVertex::layout()` return an opaque [`VertexBufferLayout`](../src/wgpu/vertex_format.rs), used in a material's `vertex_layouts` — see [Materials](./materials.md#building-a-material). `@location(0)` on a shader's vertex input has to line up with the position this layout puts things at.

## Building a mesh

[`MeshDescriptor`](../src/wgpu/mesh.rs) — a vertex list plus indices:

```rust
use pebble::wgpu::mesh::{MeshDescriptor, Vertex};

let mesh = meshes.insert("triangle", MeshDescriptor {
    vertices: vec![
        Vertex::new(glam::Vec3::new(0.0, 0.6, 0.0), glam::Vec2::ZERO, glam::Vec3::Z, glam::Vec4::new(1.0, 0.0, 0.0, 1.0)),
        // ...
    ],
    indices: vec![0, 1, 2],
});
```

`meshes.insert` returns a `Handle<MeshDescriptor>` — spawning an entity with it (and a material instance handle) as components is how a render system finds them again (see [Materials](./materials.md#rendering-with-a-material-and-instance)). Uploads to `ProcessedAssets<GPUMesh>` (`vertex_buffer`/`index_buffer`/`index_count`) automatically, through the same [asset pipeline](./the-asset-pipeline.md) as every other GPU resource.

## A custom vertex struct

`Vertex`/`InstanceVertex` cover the common case, but nothing stops a custom vertex type — build its [`VertexBufferLayout`](../src/wgpu/vertex_format.rs) by hand from opaque [`VertexAttribute`](../src/wgpu/vertex_format.rs)/[`VertexFormat`](../src/wgpu/vertex_format.rs) values (no raw `wgpu::VertexAttribute`/`vertex_attr_array!` needed):

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParticleVertex {
    position: glam::Vec3,
    size: f32,
}

fn particle_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: std::mem::size_of::<ParticleVertex>() as u64,
        step_mode: VertexStepMode::Instance,
        attributes: vec![
            VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32, offset: 12, shader_location: 1 },
        ],
    }
}
```

`offset` is the byte offset of that field within the struct (matching its `#[repr(C)]` layout); `step_mode: VertexStepMode::Instance` advances the buffer per-instance instead of per-vertex — `InstanceVertex::layout()` uses the same setting for its model-matrix columns.
