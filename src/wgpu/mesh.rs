use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
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

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            0 => Float32x3,  // position
            1 => Float32x2,  // tex_coords
            2 => Float32x3,  // normal
            3 => Float32x4,  // tangent
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
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

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            4 => Float32x4,  // model col 0
            5 => Float32x4,  // model col 1
            6 => Float32x4,  // model col 2
            7 => Float32x4,  // model col 3
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}

pub struct MeshDescriptor {
    pub vertices: Vec<Vertex>,
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
                contents: bytemuck::cast_slice(source.vertices.as_slice()),
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

#[derive(Default)]
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
