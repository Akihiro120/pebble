use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    graphics::{
        pipeline::buffers::{Buffer, BufferBuilder},
        render::Backend,
        types::{
            VertexFormat, VertexStepMode,
            flags::BufferUsages,
            pipeline_state::{VertexAttribute, VertexBufferLayout},
        },
    },
};

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: glam::Vec3,
    pub tex_coords: glam::Vec2,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec4,
}

impl Vertex {
    pub fn new(position: glam::Vec3, tex_coords: glam::Vec2, normal: glam::Vec3, tangent: glam::Vec4) -> Self {
        Self { position, tex_coords, normal, tangent }
    }

    pub fn layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: vec![
                VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                VertexAttribute { format: VertexFormat::Float32x2, offset: 12, shader_location: 1 },
                VertexAttribute { format: VertexFormat::Float32x3, offset: 20, shader_location: 2 },
                VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 3 },
            ],
        }
    }
}

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
                VertexAttribute { format: VertexFormat::Float32x4, offset: 0, shader_location: 4 },
                VertexAttribute { format: VertexFormat::Float32x4, offset: 16, shader_location: 5 },
                VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 6 },
                VertexAttribute { format: VertexFormat::Float32x4, offset: 48, shader_location: 7 },
            ],
        }
    }
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    fn validate(&self) {
        if self.vertices.is_empty() {
            tracing::warn!("Mesh::new(): no vertices — did you forget to pass them?");
        }
        if self.indices.is_empty() {
            tracing::warn!("Mesh::new(): no indices — did you forget to pass them?");
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.validate();
        assets.insert(name, self)
    }
}

pub struct GPUMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl AssetSource for Mesh {
    type Processed = GPUMesh;
}

impl Asset<Backend> for Mesh {
    type Deps<'a> = ();

    fn upload<'a>(&self, backend: &Backend, _deps: &()) -> Option<GPUMesh> {
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
