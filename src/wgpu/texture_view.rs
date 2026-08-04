use crate::wgpu::backend::WGPUBackend;

/// A `wgpu::TextureView`, opaque — the [`FrameOperations::Attachment`](crate::rendering::backend::FrameOperations::Attachment)/
/// [`DepthAttachment`](crate::rendering::backend::FrameOperations::DepthAttachment)
/// type for [`WGPUBackend`], and [`TextureBuilder::build`]'s return type.
/// Bundles the backing `wgpu::Texture` alongside the view (kept alive,
/// never otherwise accessed) — the view alone isn't enough to keep the
/// underlying resource alive for as long as it's needed.
pub struct TextureView {
    view: wgpu::TextureView,
    _texture: wgpu::Texture,
}

impl TextureView {
    pub(crate) fn raw(&self) -> &wgpu::TextureView {
        &self.view
    }
}

/// Builds a one-off GPU-side texture with no source data — a depth buffer,
/// an off-screen render target — and hands back its
/// [`TextureView`]. Unlike [`TextureDescriptor`](super::textures::TextureDescriptor),
/// which loads pixel data from a file/bytes through the asset pipeline,
/// this allocates an empty texture directly; there's nothing to upload.
///
/// ```ignore
/// let depth_view = TextureBuilder::new(backend.config.width, backend.config.height, wgpu::TextureFormat::Depth16Unorm)
///     .usage(wgpu::TextureUsages::RENDER_ATTACHMENT)
///     .build(backend);
/// ```
pub struct TextureBuilder<'a> {
    label: Option<&'a str>,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    mip_level_count: u32,
}

impl<'a> TextureBuilder<'a> {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self { label: None, width, height, format, usage: wgpu::TextureUsages::empty(), mip_level_count: 1 }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn mip_level_count(mut self, count: u32) -> Self {
        self.mip_level_count = count;
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> TextureView {
        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: self.label,
            size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: self.mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: self.usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        TextureView { view, _texture: texture }
    }
}
