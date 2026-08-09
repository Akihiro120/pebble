use crate::wgpu::{
    backend::WGPUBackend, buffer::Buffer, buffers::BindGroup, material::RenderPipeline,
    render_pass::IndexFormat, texture_format::TextureFormat,
};

/// Builds a [`RenderBundleEncoder`] — the color/depth-stencil formats and
/// sample count it (and every render pass it's later executed in via
/// [`RenderPass::execute_bundles`](super::render_pass::RenderPass::execute_bundles))
/// must match exactly. Fields are private — chain the setters below, then
/// [`build`](Self::build).
///
/// ```ignore
/// let mut encoder = RenderBundleEncoderBuilder::new()
///     .with_label("quad-bundle-encoder")
///     .with_color_formats(vec![Some(backend.surface_format())])
///     .with_sample_count(backend.sample_count())
///     .build(&backend);
/// ```
pub struct RenderBundleEncoderBuilder<'a> {
    label: Option<&'a str>,
    /// One entry per color attachment the bundle will be executed against,
    /// in the same order — `None` for an attachment slot the bundle doesn't
    /// touch.
    color_formats: Vec<Option<TextureFormat>>,
    /// `None` if the render pass(es) this bundle runs in have no depth
    /// attachment.
    depth_stencil_format: Option<TextureFormat>,
    /// Whether this bundle only reads the depth aspect (never writes it).
    depth_read_only: bool,
    /// Whether this bundle only reads the stencil aspect (never writes it).
    stencil_read_only: bool,
    /// Must match the sample count of every attachment the bundle is
    /// executed against — see [`RenderTargetTextureBuilder::with_sample_count`](super::texture_view::RenderTargetTextureBuilder::with_sample_count).
    sample_count: u32,
}

impl<'a> Default for RenderBundleEncoderBuilder<'a> {
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

impl<'a> RenderBundleEncoderBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_color_formats(mut self, formats: Vec<Option<TextureFormat>>) -> Self {
        self.color_formats = formats;
        self
    }

    pub fn with_depth_stencil_format(mut self, format: TextureFormat) -> Self {
        self.depth_stencil_format = Some(format);
        self
    }

    pub fn with_depth_read_only(mut self, read_only: bool) -> Self {
        self.depth_read_only = read_only;
        self
    }

    pub fn with_stencil_read_only(mut self, read_only: bool) -> Self {
        self.stencil_read_only = read_only;
        self
    }

    /// Must match the sample count of every attachment the bundle is
    /// executed against. `1` (no multisampling) by default.
    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    /// Logs a WARN for a bundle with neither a color nor a depth/stencil
    /// attachment configured — it wouldn't be executable against any real
    /// render pass, almost certainly a forgotten `.with_color_formats(...)`.
    fn validate(&self) {
        if self.color_formats.is_empty() && self.depth_stencil_format.is_none() {
            tracing::warn!(
                "RenderBundleEncoderBuilder: no color_formats and no depth_stencil_format — \
                 this bundle has no attachments to execute against; did you forget to call \
                 .with_color_formats(...)?"
            );
        }
    }

    /// Consume the builder and start recording a [`RenderBundleEncoder`].
    pub fn build(self, backend: &'a WGPUBackend) -> RenderBundleEncoder<'a> {
        self.validate();

        let color_formats: Vec<Option<wgpu::TextureFormat>> =
            self.color_formats.iter().map(|f| f.map(Into::into)).collect();
        let depth_stencil = self.depth_stencil_format.map(|format| wgpu::RenderBundleDepthStencil {
            format: format.into(),
            depth_read_only: self.depth_read_only,
            stencil_read_only: self.stencil_read_only,
        });
        let raw = backend.device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
            label: self.label,
            color_formats: &color_formats,
            depth_stencil,
            sample_count: self.sample_count,
            multiview: None,
        });
        RenderBundleEncoder::new(raw)
    }
}

/// Records a reusable sequence of draw calls — build via
/// [`RenderBundleEncoderBuilder`], record with the same
/// `set_pipeline`/`set_bind_group`/`set_vertex_buffer`/
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
