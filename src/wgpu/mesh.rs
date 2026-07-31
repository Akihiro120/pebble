use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
};

pub struct MeshDescriptor {
    pub vertices: Vec<u8>,
    pub indices: Vec<u32>,
}

pub struct GPUMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = MeshDescriptor;
    type Deps<'a> = ();

    fn upload<'a>(source: &MeshDescriptor, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        use wgpu::util::DeviceExt;
        let vertex_buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh Vertex Buffer"),
                contents: &source.vertices,
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = backend
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh Index Buffer"),
                contents: bytemuck::cast_slice(&source.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: source.indices.len() as u32,
        })
    }
}

pub struct MeshPlugin;
impl MeshPlugin {
    pub fn new() -> Self {
        Self
    }
}
impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AssetPlugin::<WGPUBackend, GPUMesh>::new());
    }
}
