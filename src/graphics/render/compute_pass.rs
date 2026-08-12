use crate::graphics::pipeline::{buffers::{BindGroup, Buffer}, compute::ComputePipeline};

/// An in-progress compute pass, from [`Backend::dispatch_compute`](crate::graphics::render::Backend::dispatch_compute).
pub struct ComputePass<'a> {
    raw: wgpu::ComputePass<'a>,
}

impl<'a> ComputePass<'a> {
    pub(crate) fn new(raw: wgpu::ComputePass<'a>) -> Self {
        Self { raw }
    }

    pub fn set_pipeline(&mut self, pipeline: &ComputePipeline) {
        self.raw.set_pipeline(pipeline.raw());
    }

    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.raw.set_bind_group(index, Some(bind_group.raw()), offsets);
    }

    pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.raw.dispatch_workgroups(x, y, z);
    }

    pub fn dispatch_workgroups_indirect(&mut self, indirect_buffer: &Buffer, indirect_offset: u64) {
        self.raw.dispatch_workgroups_indirect(indirect_buffer.raw(), indirect_offset);
    }
}
