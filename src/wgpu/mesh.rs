use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
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
/// Fields are private — the only way to construct one is [`MeshBuilder`]:
/// `MeshBuilder::new(vertices, indices).build()`.
pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// Builds a [`Mesh`] from a vertex/index list. Start from
/// [`new`](Self::new), then finish with
/// [`build`](Self::build)/[`build_asset`](Self::build_asset).
pub struct MeshBuilder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Logs a WARN for an empty vertex/index list — nothing would draw, and
    /// it's a far more likely sign of a forgotten argument than an
    /// intentionally invisible mesh.
    fn validate(&self) {
        if self.vertices.is_empty() {
            tracing::warn!("MeshBuilder::new(): no vertices — did you forget to pass them?");
        }
        if self.indices.is_empty() {
            tracing::warn!("MeshBuilder::new(): no indices — did you forget to pass them?");
        }
    }

    /// Consume the builder and return the finished [`Mesh`] value.
    pub fn build(self) -> Mesh {
        self.validate();
        Mesh { vertices: self.vertices, indices: self.indices }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Mesh>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Mesh>) -> Handle<Mesh> {
        let mesh = self.build();
        assets.insert(name, mesh)
    }

    /// Load the first static mesh from a glTF file.
    pub fn from_file(path: &str) -> Result<Self, super::gltf_loader::ModelLoadError> {
        let model = super::gltf_loader::load_gltf(path)?;
        let (_, mesh) = model.static_meshes.into_iter().next()
            .ok_or_else(|| super::gltf_loader::ModelLoadError::MissingData("no static mesh in file".to_string()))?;
        Ok(Self { vertices: mesh.vertices, indices: mesh.indices })
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

impl AssetSource for Mesh {
    type Processed = GPUMesh;
}

impl Asset<WGPUBackend> for Mesh {
    type Deps<'a> = ();

    fn upload<'a>(&self, backend: &WGPUBackend, _deps: &()) -> Option<GPUMesh> {
        let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(self.vertices.as_slice()))
            .with_label("Mesh Vertex Buffer")
            .with_usage(BufferUsages::VERTEX)
            .build(backend);
        let index_buffer = BufferBuilder::with_data(bytemuck::cast_slice(&self.indices))
            .with_label("Mesh Index Buffer")
            .with_usage(BufferUsages::INDEX)
            .build(backend);
        Some(GPUMesh {
            vertex_buffer,
            index_buffer,
            index_count: self.indices.len() as u32,
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`Mesh`] asset pipeline. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MeshPlugin, Mesh
}
