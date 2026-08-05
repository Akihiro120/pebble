use crate::wgpu::{
    buffer::Buffer, buffers::BindGroup, material::RenderPipeline, render_pass::IndexFormat,
    texture_format::TextureFormat,
};

/// Describes a [`RenderBundleEncoder`] — the color/depth-stencil formats and
/// sample count it (and every render pass it's later executed in via
/// [`RenderPass::execute_bundles`](super::render_pass::RenderPass::execute_bundles))
/// must match exactly.
pub struct RenderBundleEncoderDescriptor<'a> {
    pub label: Option<&'a str>,
    /// One entry per color attachment the bundle will be executed against,
    /// in the same order — `None` for an attachment slot the bundle doesn't
    /// touch.
    pub color_formats: Vec<Option<TextureFormat>>,
    /// `None` if the render pass(es) this bundle runs in have no depth
    /// attachment.
    pub depth_stencil_format: Option<TextureFormat>,
    /// Whether this bundle only reads the depth aspect (never writes it).
    pub depth_read_only: bool,
    /// Whether this bundle only reads the stencil aspect (never writes it).
    pub stencil_read_only: bool,
    /// Must match the sample count of every attachment the bundle is
    /// executed against — see [`TextureBuilder::sample_count`](super::texture_view::TextureBuilder::sample_count).
    pub sample_count: u32,
}

impl<'a> Default for RenderBundleEncoderDescriptor<'a> {
    fn default() -> Self {
        Self {
            label: None,
            color_formats: Vec::new(),
            depth_stencil_format: None,
            depth_read_only: false,
            stencil_read_only: false,
            sample_count: 1,
        }
    }
}

/// Records a reusable sequence of draw calls — build via
/// [`WGPUBackend::create_render_bundle_encoder`](super::backend::WGPUBackend::create_render_bundle_encoder),
/// record with the same `set_pipeline`/`set_bind_group`/`set_vertex_buffer`/
/// `set_index_buffer`/`draw`/`draw_indexed` shape as
/// [`RenderPass`](super::render_pass::RenderPass), then
/// [`finish`](Self::finish) into a [`RenderBundle`]. Re-executing a bundle
/// via [`RenderPass::execute_bundles`](super::render_pass::RenderPass::execute_bundles)
/// is often cheaper than re-recording the same draws by hand every frame —
/// worth it once you have many draw calls that don't change pipeline/bind
/// group/buffers from one frame to the next (static scene geometry, say).
pub struct RenderBundleEncoder<'a> {
    raw: wgpu::RenderBundleEncoder<'a>,
}

impl<'a> RenderBundleEncoder<'a> {
    pub(crate) fn new(raw: wgpu::RenderBundleEncoder<'a>) -> Self {
        Self { raw }
    }

    pub fn set_pipeline(&mut self, pipeline: &'a RenderPipeline) {
        self.raw.set_pipeline(pipeline.raw());
    }

    /// `offsets` is the dynamic-offset slice for any dynamic-offset entries
    /// in this bind group's layout — see
    /// [`RenderPass::set_bind_group`](super::render_pass::RenderPass::set_bind_group).
    pub fn set_bind_group(&mut self, index: u32, bind_group: &'a BindGroup, offsets: &[u32]) {
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

    /// Stops recording and returns the replayable [`RenderBundle`].
    pub fn finish(self, label: Option<&str>) -> RenderBundle {
        RenderBundle(self.raw.finish(&wgpu::RenderBundleDescriptor { label }))
    }
}

/// A pre-recorded, replayable sequence of draw calls — built via
/// [`RenderBundleEncoder::finish`], replayed via
/// [`RenderPass::execute_bundles`](super::render_pass::RenderPass::execute_bundles).
/// There's no way to reach the underlying `wgpu::RenderBundle` from outside
/// this crate.
pub struct RenderBundle(wgpu::RenderBundle);

impl RenderBundle {
    pub(crate) fn raw(&self) -> &wgpu::RenderBundle {
        &self.0
    }
}
