# Meshes and Vertices

`Mesh<V>` is a vertex + index buffer asset, generic over vertex type — `V` defaults to the built-in `Vertex` (position, UV, normal, tangent), but any `bytemuck::Pod` struct works:

```rust,ignore
fn setup(backend: Read<Backend>, mut meshes: Write<Assets<Mesh>>) {
    let vertices = vec![/* Vertex { .. } */];
    let indices = vec![0, 1, 2, /* ... */];
    let handle = Mesh::new(vertices, indices).build_asset("cube", &mut meshes);
}
```

The uploaded `GPUMesh` has public `vertex_buffer`/`index_buffer` (both plain [`Buffer`](./buffers.md)) and `index_count`, ready to pass to `set_vertex_buffer`/`set_index_buffer`/`draw_indexed` — see [Recording a Render Pass](./rendering-pass-recording.md).

## CPU-side access

`vertices()`/`indices()` return the CPU-side source data — e.g. for building a collision mesh from the same data used to render it. `release_cpu_data()` frees that copy once you're done reading it, but unlike other asset types, a released mesh can *never* be re-uploaded — if the GPU backend is ever lost and recreated afterward, this mesh logs an error and simply stays not-ready forever. Only call it if that's acceptable for this particular mesh.

## Custom vertex types

```rust,ignore
#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct MyVertex {
    position: glam::Vec3,
    color: glam::Vec4,
}

impl MyVertex {
    fn layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<MyVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: vec![
                VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                VertexAttribute { format: VertexFormat::Float32x4, offset: 12, shader_location: 1 },
            ],
        }
    }
}
```

Pass `MyVertex::layout()` to `Material::with_vertex_layouts` (see [Materials](./materials.md)) so the pipeline's vertex stage matches.

## Instancing

`InstanceVertex` carries just a model matrix, for instanced draws — bind it alongside a regular vertex buffer at `VertexStepMode::Instance`, then pass an instance range to `draw`/`draw_indexed` instead of `0..1`.
