use crate::{assets::upload::Asset, wgpu::{backend::WGPUBackend, buffers::BufferBuilder}};

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

/// Source data for [`GPUMesh`]: a plain vertex/index list, uploaded as-is.
pub struct MeshDescriptor {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// A mesh uploaded to the GPU. `index_buffer`/`index_count` must stay in
/// sync if you ever mutate one after construction — there's no invariant
/// check, so a mismatched pair silently draws garbage or the wrong index
/// range.
pub struct GPUMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Asset<WGPUBackend> for GPUMesh {
    type Source = MeshDescriptor;
    type Deps<'a> = ();

    fn upload<'a>(source: &MeshDescriptor, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let vertex_buffer = BufferBuilder::new()
            .label("Mesh Vertex Buffer")
            .usage(wgpu::BufferUsages::VERTEX)
            .data(bytemuck::cast_slice(source.vertices.as_slice()))
            .build(&backend.device);
        let index_buffer = BufferBuilder::new()
            .label("Mesh Index Buffer")
            .usage(wgpu::BufferUsages::INDEX)
            .data(bytemuck::cast_slice(&source.indices))
            .build(&backend.device);
        Some(Self {
            vertex_buffer,
            index_buffer,
            index_count: source.indices.len() as u32,
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUMesh`] asset pipeline (`Assets<MeshDescriptor>` →
    /// `ProcessedAssets<GPUMesh>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MeshPlugin, GPUMesh
}
