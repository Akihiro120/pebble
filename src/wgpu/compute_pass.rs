use crate::wgpu::{buffers::BindGroup, compute::ComputePipeline};

/// A compute pass, opaque — the value [`CommandEncoder::compute_pass`]/
/// [`WGPUFrame::compute_pass`](super::backend::WGPUFrame::compute_pass)
/// hand back. There's no way to reach the underlying `wgpu::ComputePass`
/// from outside this crate.
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

    /// `offsets` is the dynamic-offset slice for any dynamic-offset entries
    /// in this bind group's layout, in the order they appear — empty if
    /// none. See [`RenderPass::set_bind_group`](super::render_pass::RenderPass::set_bind_group).
    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.raw.set_bind_group(index, Some(bind_group.raw()), offsets);
    }

    pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.raw.dispatch_workgroups(x, y, z);
    }
}

/// A command encoder, opaque — built only via
/// [`WGPUBackend::create_command_encoder`](super::backend::WGPUBackend::create_command_encoder).
/// Its only job is starting a [`ComputePass`]; submit it when done via
/// [`WGPUBackend::submit`](super::backend::WGPUBackend::submit). Render
/// passes don't need this — [`ActiveFrame::begin_pass`](crate::rendering::active_frame::ActiveFrame::begin_pass)
/// manages its own frame-tied encoder internally.
pub struct CommandEncoder {
    raw: wgpu::CommandEncoder,
}

impl CommandEncoder {
    pub(crate) fn new(raw: wgpu::CommandEncoder) -> Self {
        Self { raw }
    }

    pub fn compute_pass(&mut self, label: Option<&str>) -> ComputePass<'_> {
        let raw = self.raw.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            timestamp_writes: None,
        });
        ComputePass::new(raw)
    }

    pub(crate) fn into_raw(self) -> wgpu::CommandEncoder {
        self.raw
    }
}
