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

/// Same 4 fields as [`Vertex`](super::mesh::Vertex), plus skinning data.
///
/// `_pad` exists purely so this type has no *implicit* padding: `glam::Vec4`
/// is 16-byte aligned (SIMD-backed), which forces this whole struct's
/// alignment to 16 — without an explicit trailing field to account for the
/// last 8 bytes, the compiler would insert *invisible* padding there
/// instead, and `#[derive(bytemuck::Pod)]` correctly refuses to compile a
/// type with any padding it can't account for byte-for-byte.
#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    pub position: glam::Vec3,
    pub tex_coords: glam::Vec2,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec4,
    /// Up to 4 joint indices this vertex is bound to, paired positionally
    /// with `joint_weights`. `u16`, not `u32` or `u8`: glTF's `JOINTS_n`
    /// accessor is spec-limited to `UNSIGNED_BYTE`/`UNSIGNED_SHORT` (never
    /// `UNSIGNED_INT`), so `u16` already covers every legal glTF skeleton
    /// (up to 65535 joints) losslessly, at half the bytes of `u32`.
    /// [`gltf_loader::load_gltf`](super::gltf_loader::load_gltf) upconverts
    /// `u8`-sourced `JOINTS_0` data.
    pub joint_indices: [u16; 4],
    pub joint_weights: [f32; 4],
    _pad: [u32; 2],
}

impl SkinnedVertex {
    pub fn new(
        position: glam::Vec3,
        tex_coords: glam::Vec2,
        normal: glam::Vec3,
        tangent: glam::Vec4,
        joint_indices: [u16; 4],
        joint_weights: [f32; 4],
    ) -> Self {
        Self {
            position,
            tex_coords,
            normal,
            tangent,
            joint_indices,
            joint_weights,
            _pad: [0; 2],
        }
    }

    /// Occupies locations 0–3 (same meaning as
    /// [`Vertex`](super::mesh::Vertex)'s own 0–3) and 8–9 — deliberately
    /// skipping 4–7 (reserved for [`InstanceVertex`](super::mesh::InstanceVertex))
    /// so a skinned mesh can still be instanced in one pipeline without
    /// either layout changing. WGSL side: all integer vertex formats widen
    /// to `vec4<u32>` regardless of source width, so declare
    /// `@location(8) joint_indices: vec4<u32>` and
    /// `@location(9) joint_weights: vec4<f32>`.
    pub fn layout() -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinnedVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: vec![
                VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 }, // position
                VertexAttribute { format: VertexFormat::Float32x2, offset: 12, shader_location: 1 }, // tex_coords
                VertexAttribute { format: VertexFormat::Float32x3, offset: 20, shader_location: 2 }, // normal
                VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 3 }, // tangent
                VertexAttribute { format: VertexFormat::Uint16x4, offset: 48, shader_location: 8 }, // joint_indices
                VertexAttribute { format: VertexFormat::Float32x4, offset: 56, shader_location: 9 }, // joint_weights
            ],
        }
    }
}

/// Source data for [`GPUSkinnedMesh`]: a plain vertex/index list, uploaded
/// as-is. Fields are private — the only way to construct one is
/// [`SkinnedMeshBuilder`]: `SkinnedMeshBuilder::new(vertices, indices).build()`.
/// Usually obtained via [`gltf_loader::load_gltf`](super::gltf_loader::load_gltf)
/// rather than hand-authored.
pub struct SkinnedMesh {
    vertices: Vec<SkinnedVertex>,
    indices: Vec<u32>,
}

/// Builds a [`SkinnedMesh`] from a vertex/index list. Start from
/// [`new`](Self::new), then finish with
/// [`build`](Self::build)/[`build_asset`](Self::build_asset).
pub struct SkinnedMeshBuilder {
    vertices: Vec<SkinnedVertex>,
    indices: Vec<u32>,
}

impl SkinnedMeshBuilder {
    pub fn new(vertices: Vec<SkinnedVertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Same empty-vertices/empty-indices checks as
    /// [`MeshBuilder`](super::mesh::MeshBuilder), plus: warns if any
    /// vertex's `joint_weights` don't sum to ~1.0 (±0.01) — the usual
    /// symptom of un-normalized weights, most likely from a hand-authored
    /// `SkinnedMesh` that skipped `load_gltf` (which always normalizes).
    fn validate(&self) {
        if self.vertices.is_empty() {
            tracing::warn!("SkinnedMeshBuilder::new(): no vertices — did you forget to pass them?");
        }
        if self.indices.is_empty() {
            tracing::warn!("SkinnedMeshBuilder::new(): no indices — did you forget to pass them?");
        }
        for (i, vertex) in self.vertices.iter().enumerate() {
            let sum: f32 = vertex.joint_weights.iter().sum();
            if (sum - 1.0).abs() > 0.01 {
                tracing::warn!(
                    "SkinnedMeshBuilder: vertex {i}'s joint_weights sum to {sum}, not ~1.0 — did \
                     you forget to normalize them?"
                );
            }
        }
    }

    /// Consume the builder and return the finished [`SkinnedMesh`] value.
    pub fn build(self) -> SkinnedMesh {
        self.validate();
        SkinnedMesh { vertices: self.vertices, indices: self.indices }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<SkinnedMesh>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<SkinnedMesh>) -> Handle<SkinnedMesh> {
        let mesh = self.build();
        assets.insert(name, mesh)
    }
}

/// A skinned mesh uploaded to the GPU. `index_buffer`/`index_count` must
/// stay in sync if you ever mutate one after construction — same caveat as
/// [`GPUMesh`](super::mesh::GPUMesh).
pub struct GPUSkinnedMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl Asset<WGPUBackend> for GPUSkinnedMesh {
    type Source = SkinnedMesh;
    type Deps<'a> = ();

    fn upload<'a>(source: &SkinnedMesh, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(source.vertices.as_slice()))
            .label("SkinnedMesh Vertex Buffer")
            .usage(BufferUsages::VERTEX)
            .build(backend);
        let index_buffer = BufferBuilder::with_data(bytemuck::cast_slice(&source.indices))
            .label("SkinnedMesh Index Buffer")
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
    /// Registers the [`GPUSkinnedMesh`] asset pipeline (`Assets<SkinnedMesh>`
    /// → `ProcessedAssets<GPUSkinnedMesh>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    SkinnedMeshPlugin, GPUSkinnedMesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(joint_weights: [f32; 4]) -> SkinnedVertex {
        SkinnedVertex::new(
            glam::Vec3::ZERO,
            glam::Vec2::ZERO,
            glam::Vec3::Z,
            glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
            [0, 0, 0, 0],
            joint_weights,
        )
    }

    #[test]
    fn layout_matches_the_verified_byte_offsets() {
        assert_eq!(std::mem::size_of::<SkinnedVertex>(), 80);
        let layout = SkinnedVertex::layout();
        assert_eq!(layout.array_stride, 80);
        assert_eq!(layout.attributes.len(), 6);
        assert_eq!(layout.attributes[4].offset, 48);
        assert_eq!(layout.attributes[4].shader_location, 8);
        assert_eq!(layout.attributes[5].offset, 56);
        assert_eq!(layout.attributes[5].shader_location, 9);
    }

    #[test]
    fn build_does_not_panic_regardless_of_weight_sum() {
        // Mismatched weights are a tracing::warn!, not a hard failure —
        // matching every other validate() in this codebase.
        let mesh = SkinnedMeshBuilder::new(vec![vertex([0.5, 0.0, 0.0, 0.0])], vec![0]).build();
        assert_eq!(mesh.vertices.len(), 1);
    }

    #[test]
    fn build_with_normalized_weights_does_not_panic() {
        let mesh = SkinnedMeshBuilder::new(vec![vertex([1.0, 0.0, 0.0, 0.0])], vec![0]).build();
        assert_eq!(mesh.indices.len(), 1);
    }
}
