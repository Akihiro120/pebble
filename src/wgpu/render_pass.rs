use crate::wgpu::{buffer::Buffer, buffers::BindGroup, material::RenderPipeline};

/// A render pass, opaque — the value [`ActiveFrame::begin_pass`](crate::rendering::active_frame::ActiveFrame::begin_pass)/
/// [`render_context`](crate::rendering::active_frame::ActiveFrame::render_context)
/// hands back for [`WGPUBackend`](super::backend::WGPUBackend). There's no
/// way to reach the underlying `wgpu::RenderPass` from outside this crate —
/// every draw-time operation is a method here instead.
pub struct RenderPass<'a> {
    raw: wgpu::RenderPass<'a>,
}

impl<'a> RenderPass<'a> {
    pub(crate) fn new(raw: wgpu::RenderPass<'a>) -> Self {
        Self { raw }
    }

    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        self.raw.set_pipeline(pipeline.raw());
    }

    /// `offsets` is the dynamic-offset slice for any
    /// [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer)/
    /// [`dynamic_storage_buffer`](super::binding::BindingKind::dynamic_storage_buffer)
    /// entries in this bind group's layout, in the order they appear —
    /// empty if none.
    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.raw.set_bind_group(index, Some(bind_group.raw()), offsets);
    }

    /// Binds `buffer` in its entirety at vertex slot `slot`.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &'a Buffer) {
        self.raw.set_vertex_buffer(slot, buffer.raw().slice(..));
    }

    /// Binds `buffer` in its entirety as the index buffer.
    pub fn set_index_buffer(&mut self, buffer: &'a Buffer, format: IndexFormat) {
        self.raw.set_index_buffer(buffer.raw().slice(..), format.into());
    }

    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.raw.draw(vertices, instances);
    }

    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.raw.draw_indexed(indices, base_vertex, instances);
    }

    /// Same as [`draw`](Self::draw), but the vertex/instance counts come
    /// from a [`DrawIndirectArgs`] value already written into
    /// `indirect_buffer` at `indirect_offset` bytes — for a GPU-driven draw
    /// count (culling compaction, particle counts, ...) the CPU never reads
    /// back.
    pub fn draw_indirect(&mut self, indirect_buffer: &'a Buffer, indirect_offset: u64) {
        self.raw.draw_indirect(indirect_buffer.raw(), indirect_offset);
    }

    /// Same as [`draw_indexed`](Self::draw_indexed), but the counts come
    /// from a [`DrawIndexedIndirectArgs`] value already written into
    /// `indirect_buffer` at `indirect_offset` bytes.
    pub fn draw_indexed_indirect(&mut self, indirect_buffer: &'a Buffer, indirect_offset: u64) {
        self.raw.draw_indexed_indirect(indirect_buffer.raw(), indirect_offset);
    }

    /// Replays every bundle in `bundles`, in order, as if their recorded
    /// commands had been issued directly against this pass.
    pub fn execute_bundles(&mut self, bundles: &[&super::render_bundle::RenderBundle]) {
        self.raw.execute_bundles(bundles.iter().map(|b| b.raw()));
    }

    /// Drops the borrow-checker tie to whatever this pass's encoder was
    /// borrowed from, without actually extending its real lifetime — see
    /// `wgpu::RenderPass::forget_lifetime`'s own docs for the safety
    /// contract. `pub(crate)` only, and only compiled in with the
    /// `profiler` feature: needed by [`profiler`](super::profiler) to
    /// satisfy `egui_wgpu::Renderer::render`'s `RenderPass<'static>`
    /// requirement, not something ordinary rendering code needs.
    #[cfg(feature = "profiler")]
    pub(crate) fn forget_lifetime(self) -> RenderPass<'static> {
        RenderPass { raw: self.raw.forget_lifetime() }
    }

    /// `pub(crate)` escape hatch for internal code (the profiler overlay)
    /// that hands this pass to a third-party renderer (`egui_wgpu`)
    /// expecting a raw `wgpu::RenderPass`.
    #[cfg(feature = "profiler")]
    pub(crate) fn raw_mut(&mut self) -> &mut wgpu::RenderPass<'a> {
        &mut self.raw
    }
}

/// Index buffer element width — mirrors `wgpu::IndexFormat` (which has
/// exactly these two variants).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IndexFormat {
    Uint16,
    Uint32,
}

impl From<IndexFormat> for wgpu::IndexFormat {
    fn from(format: IndexFormat) -> Self {
        match format {
            IndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
            IndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
        }
    }
}

/// The argument layout [`RenderPass::draw_indirect`] expects at
/// `indirect_offset` in the indirect buffer — mirrors `wgpu::DrawIndirectArgs`
/// field-for-field, so writing one via [`as_bytes`](Self::as_bytes) into a
/// buffer built with [`BufferUsages::INDIRECT`](super::flags::BufferUsages::INDIRECT)
/// produces the exact same bytes wgpu itself would.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndirectArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl DrawIndirectArgs {
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

/// The argument layout [`RenderPass::draw_indexed_indirect`] expects —
/// mirrors `wgpu::DrawIndexedIndirectArgs` field-for-field. See
/// [`DrawIndirectArgs`].
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndexedIndirectArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

impl DrawIndexedIndirectArgs {
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}
