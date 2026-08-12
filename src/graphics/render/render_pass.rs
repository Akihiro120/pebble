use crate::graphics::{pipeline::{buffers::{BindGroup, Buffer}, material::RenderPipeline}, types::IndexFormat};

/// An in-progress render pass, from [`Frame::begin`](crate::graphics::render::frame::Frame::begin).
/// Method names mirror `wgpu`'s own render pass API.
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

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.raw.set_bind_group(index, Some(bind_group.raw()), offsets);
    }

    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &'a Buffer) {
        self.raw.set_vertex_buffer(slot, buffer.raw().slice(..));
    }

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

    pub fn draw_indirect(&mut self, indirect_buffer: &'a Buffer, indirect_offset: u64) {
        self.raw.draw_indirect(indirect_buffer.raw(), indirect_offset);
    }

    pub fn draw_indexed_indirect(&mut self, indirect_buffer: &'a Buffer, indirect_offset: u64) {
        self.raw.draw_indexed_indirect(indirect_buffer.raw(), indirect_offset);
    }
}

/// The exact byte layout `draw_indirect` expects in its indirect buffer —
/// write this (via `bytemuck::bytes_of`/[`as_bytes`](Self::as_bytes)) to a
/// buffer built with `INDIRECT` usage.
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

/// The exact byte layout `draw_indexed_indirect` expects — same idea as
/// [`DrawIndirectArgs`], for the indexed draw variant.
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
