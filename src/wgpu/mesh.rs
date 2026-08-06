use crate::{
    assets::{handle::Handle, storage::Assets, upload::Asset},
    wgpu::{
        backend::WGPUBackend,
        buffer::Buffer,
        buffers::BufferBuilder,
        flags::BufferUsages,
        vertex_format::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode},
    },
};

/// Standard per-vertex data: position, UV, normal, and tangent (`w` is the
/// bitangent handedness sign, ±1 — cross `normal` with `tangent.xyz` and
/// scale by `tangent.w` to get the bitangent).
///
/// Occupies vertex buffer locations 0–3 — [`InstanceVertex::layout`]
/// deliberately starts at location 4 to leave room for this, so pairing
/// them in the same pipeline doesn't collide. Adding a 5th attribute here
/// would need a matching shift there.
#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub tex_coords: glam::Vec2,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec4,
}

impl Vertex {
    pub fn new(
        position: glam::Vec3,
        tex_coords: glam::Vec2,
        normal: glam::Vec3,
        tangent: glam::Vec4,
    ) -> Self {
        Self {
            position,
            tex_coords,
            normal,
            tangent,
        }
    }

    pub fn layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: vec![
                VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 }, // position
                VertexAttribute { format: VertexFormat::Float32x2, offset: 12, shader_location: 1 }, // tex_coords
                VertexAttribute { format: VertexFormat::Float32x3, offset: 20, shader_location: 2 }, // normal
                VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 3 }, // tangent
            ],
        }
    }
}

/// Per-instance vertex data. Carries a model matrix as four `Vec4` columns,
/// passed as vertex attributes with `step_mode: Instance`.
#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceVertex {
    pub model: glam::Mat4,
}

impl InstanceVertex {
    pub fn new(model: glam::Mat4) -> Self {
        Self { model }
    }

    pub fn layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceVertex>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute { format: VertexFormat::Float32x4, offset: 0, shader_location: 4 }, // model col 0
                VertexAttribute { format: VertexFormat::Float32x4, offset: 16, shader_location: 5 }, // model col 1
                VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 6 }, // model col 2
                VertexAttribute { format: VertexFormat::Float32x4, offset: 48, shader_location: 7 }, // model col 3
            ],
        }
    }
}

/// Source data for [`GPUMesh`]: a plain vertex/index list, uploaded as-is.
/// Fields are private — build one via [`Mesh::new`] rather than as a struct
/// literal.
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Logs a WARN for an empty vertex/index list — nothing would draw, and
    /// it's a far more likely sign of a forgotten argument than an
    /// intentionally invisible mesh.
    fn validate(&self) {
        if self.vertices.is_empty() {
            tracing::warn!("Mesh::new(): no vertices — did you forget to pass them?");
        }
        if self.indices.is_empty() {
            tracing::warn!("Mesh::new(): no indices — did you forget to pass them?");
        }
    }

    /// Consume the builder and return the finished [`Mesh`] value.
    pub fn build(self) -> Self {
        self.validate();
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Mesh>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        self.validate();
        assets.insert(name, self)
    }
}

/// A mesh uploaded to the GPU. `index_buffer`/`index_count` must stay in
/// sync if you ever mutate one after construction — there's no invariant
/// check, so a mismatched pair silently draws garbage or the wrong index
/// range.
pub struct GPUMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = Mesh;
    type Deps<'a> = ();

    fn upload<'a>(source: &Mesh, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(source.vertices.as_slice()))
            .label("Mesh Vertex Buffer")
            .usage(BufferUsages::VERTEX)
            .build(backend);
        let index_buffer = BufferBuilder::with_data(bytemuck::cast_slice(&source.indices))
            .label("Mesh Index Buffer")
            .usage(BufferUsages::INDEX)
            .build(backend);
        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: source.indices.len() as u32,
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUMesh`] asset pipeline (`Assets<Mesh>` →
    /// `ProcessedAssets<GPUMesh>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MeshPlugin, GPUMesh
}
