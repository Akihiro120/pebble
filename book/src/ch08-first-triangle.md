# Your First Triangle

Time to draw something. This chapter builds a mesh and a material from scratch and gets a solid-colored triangle on screen — no texture yet, that's the next chapter.

## The shader

`MaterialDescriptor` takes one WGSL string covering both the vertex and fragment stage:

```rust
const SHADER: &str = r#"
@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(pos, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.5, 0.2, 1.0);
}
"#;
```

`@location(0)` on the vertex input has to line up with the vertex buffer layout you give `MaterialDescriptor.vertex_layouts` — that's `Vertex::layout()`, covered next.

## Vertex data

`pebble::wgpu::mesh::Vertex` is a fixed layout: position, texture coordinates, normal, and tangent — everything a lit, textured mesh needs. This shader only reads position, but the vertex buffer still needs all four fields populated, since the buffer's layout is fixed regardless of what any one shader chooses to read:

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

## Setup: inserting the mesh and material

Everything from here on happens in a `.once()` system — see Chapter 2 if you skipped Part I. `Res<WGPUBackend>` (the built-in backend, not your own) is a hard requirement here purely so `setup` waits until the GPU device exists before reading `backend.surface_format()`; nothing in `setup` needs to touch the device directly.

```rust
use pebble::wgpu::{
    backend::WGPUBackend,
    material::{ColorTargetState, MaterialDescriptor},
    mesh::MeshDescriptor,
};

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<MeshDescriptor>>,
    mut materials: ResMut<Assets<MaterialDescriptor<'static>>>,
    backend: Res<WGPUBackend>,
) -> Option<()> {
    let triangle = meshes.insert(
        "triangle",
        MeshDescriptor {
            vertices: triangle_vertices(),
            indices: vec![0, 1, 2],
        },
    );

    let material = materials.insert(
        "solid_orange",
        MaterialDescriptor {
            label: Some("solid-orange"),
            shader_source: SHADER,
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: vec![Vertex::layout()],
            entries: vec![],   // no textures/uniforms yet — see the next chapter
            own_group: None,   // ...so there's no bind group at all for this material
            targets: vec![ColorTargetState {
                format: backend.surface_format(),
                blend: None,
                write_mask: Default::default(),
            }],
            ..Default::default()
        },
    );

    commands.spawn((triangle, material));

    Some(())
}
```

`meshes.insert`/`materials.insert` return `Handle<MeshDescriptor>`/`Handle<MaterialDescriptor>` — spawning an entity with both handles as components is how the render system (next) finds them again. `entries: vec![]` plus `own_group: None` together mean "this material has no bind group at all" — valid, since the shader above doesn't declare a `@group` either. The moment a shader wants a texture or a uniform, both of those need to change; that's the whole subject of the next chapter.

`..Default::default()` fills in the rest: `cull_mode: Some(Face::Back)`, no depth testing, fill polygon mode. [Camera, Depth, and Lazy Resources](./ch10-camera-and-depth.md) is where `depth` stops being `None`.

## Rendering

```rust
use pebble::wgpu::mesh::GPUMesh;
use pebble::wgpu::material::GPUMaterial;
use pebble::wgpu::render_pass::IndexFormat;

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    materials: Res<ProcessedAssets<GPUMaterial>>,
    meshes: Res<ProcessedAssets<GPUMesh>>,
    mut query: Query<(&Handle<MeshDescriptor>, &Handle<MaterialDescriptor<'static>>)>,
) {
    let Some(mut active) = frame.active() else {
        return;
    };
    let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);

    for (mesh_handle, material_handle) in query.iter() {
        let Some(mesh) = meshes.get(mesh_handle.id) else { continue };
        let Some(material) = materials.get(material_handle.id) else { continue };

        pass.set_pipeline(&material.pipeline);
        pass.set_vertex_buffer(0, &mesh.vertex_buffer);
        pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
```

Two things worth noticing:

- **`Res<ProcessedAssets<GPUMesh>>`, not `Assets<MeshDescriptor>`.** `render` reads the *uploaded* GPU-side objects (built by the asset pipeline from Chapter 6), not the CPU-side descriptors that produced them.
- **`meshes.get`/`materials.get` return `Option`, and a miss just `continue`s.** The asset might genuinely not be uploaded yet on the very first few frames — same "not ready yet, not an error" shape as every other async boundary in this book.

Run it, and you get an orange triangle over a dark background. Everything from here (Chapter 9 onward) is additive on top of this same skeleton: a bind group for a texture, a camera bind group, a depth attachment.
