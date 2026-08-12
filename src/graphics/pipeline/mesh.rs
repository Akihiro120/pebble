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

pub struct Mesh<V: bytemuck::Pod = Vertex> {
    vertices: Option<Vec<V>>,
    indices: Option<Vec<u32>>,
}

impl<V: bytemuck::Pod> Mesh<V> {
    pub fn new(vertices: Vec<V>, indices: Vec<u32>) -> Self {
        Self { vertices: Some(vertices), indices: Some(indices) }
    }

    fn validate(&self) {
        if self.vertices.as_ref().is_none_or(|v| v.is_empty()) {
            tracing::warn!("Mesh::new(): no vertices — did you forget to pass them?");
        }
        if self.indices.as_ref().is_none_or(|i| i.is_empty()) {
            tracing::warn!("Mesh::new(): no indices — did you forget to pass them?");
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Mesh<V>>) -> Handle<Mesh<V>> {
        self.validate();
        assets.insert(name, self)
    }

    // CPU-side vertices — for e.g. building a collision mesh from the same
    // source data used to upload the GPU buffer. None once
    // release_cpu_data() has been called.
    pub fn vertices(&self) -> Option<&[V]> {
        self.vertices.as_deref()
    }

    pub fn indices(&self) -> Option<&[u32]> {
        self.indices.as_deref()
    }

    // Frees the CPU-side copy — call once you've read whatever you needed
    // from vertices()/indices() (e.g. after building a collision mesh) and
    // don't need it kept around just to sit in memory unused.
    //
    // Trade-off: the unified asset model normally keeps a mesh's source
    // around so it can be re-uploaded if the GPU backend is ever lost and
    // recreated. After this call, upload() has nothing left to rebuild
    // from — if that happens, this mesh logs an error and simply never
    // becomes ready again. Only call this once you're sure that's
    // acceptable for this particular mesh.
    pub fn release_cpu_data(&mut self) {
        self.vertices = None;
        self.indices = None;
    }
}

pub struct GPUMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl<V: bytemuck::Pod + 'static> AssetSource for Mesh<V> {
    type Processed = GPUMesh;
}

impl<V: bytemuck::Pod + 'static> Asset<Backend> for Mesh<V> {
    type Deps<'a> = ();

    fn upload<'a>(&self, backend: &Backend, _deps: &()) -> Option<GPUMesh> {
        let (Some(vertices), Some(indices)) = (&self.vertices, &self.indices) else {
            tracing::error!(
                "Mesh::upload: CPU vertex/index data was released via release_cpu_data() and \
                 the GPU resource needs to be (re)built — this mesh can never become ready"
            );
            return None;
        };

        let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(vertices.as_slice()))
            .with_label("Mesh Vertex Buffer")
            .with_usage(BufferUsages::VERTEX)
            .build(backend);
        let index_buffer = BufferBuilder::with_data(bytemuck::cast_slice(indices))
            .with_label("Mesh Index Buffer")
            .with_usage(BufferUsages::INDEX)
            .build(backend);
        Some(GPUMesh {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        })
    }
}
