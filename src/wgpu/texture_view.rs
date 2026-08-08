use crate::wgpu::backend::WGPUBackend;
use crate::wgpu::flags::TextureUsages;
use crate::wgpu::texture_format::TextureFormat;
use crate::wgpu::textures::check_texture_dimensions;

/// A `wgpu::TextureView`, opaque — the [`FrameOperations::Attachment`](crate::rendering::backend::FrameOperations::Attachment)/
/// [`DepthAttachment`](crate::rendering::backend::FrameOperations::DepthAttachment)
/// type for [`WGPUBackend`], and [`RenderTargetTextureBuilder::build`]'s return type.
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

    /// Wraps an already-created `wgpu::TextureView` onto an existing
    /// texture (a `wgpu::Texture` is a cheap, `Arc`-backed handle, so
    /// `texture` is typically `.clone()`d off whatever already owns it) —
    /// used by [`GPUCubemap::face_attachment`](super::cubemap::GPUCubemap::face_attachment)
    /// for a render target into one face of an existing texture, as
    /// opposed to [`RenderTargetTextureBuilder::build`] which allocates a brand new one.
    pub(crate) fn from_raw(view: wgpu::TextureView, texture: wgpu::Texture) -> Self {
        Self { view, _texture: texture }
    }
}

/// Builds a one-off GPU-side texture with no source data — a depth buffer,
/// an off-screen render target — and hands back its
/// [`TextureView`]. Unlike [`Texture`](super::textures::Texture),
/// which loads pixel data from a file/bytes through the asset pipeline,
/// this allocates an empty texture directly; there's nothing to upload.
///
/// ```ignore
/// let depth_view = RenderTargetTextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
///     .usage(TextureUsages::RENDER_ATTACHMENT)
///     .build(backend);
/// ```
pub struct RenderTargetTextureBuilder<'a> {
    label: Option<&'a str>,
    width: u32,
    height: u32,
    format: TextureFormat,
    usage: TextureUsages,
    mip_level_count: u32,
    sample_count: u32,
}

impl<'a> RenderTargetTextureBuilder<'a> {
    pub fn new(width: u32, height: u32, format: TextureFormat) -> Self {
        Self {
            label: None,
            width,
            height,
            format,
            usage: TextureUsages::empty(),
            mip_level_count: 1,
            sample_count: 1,
        }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn usage(mut self, usage: TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn mip_level_count(mut self, count: u32) -> Self {
        self.mip_level_count = count;
        self
    }

    /// Multisample count — must match whatever this texture is used
    /// alongside (a depth attachment paired with an MSAA color target needs
    /// the same count as [`WGPUBackend::sample_count`], say). `1` (no
    /// multisampling) by default.
    pub fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> TextureView {
        check_texture_dimensions(&backend.device, "RenderTargetTextureBuilder", self.width, self.height);

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: self.label,
            size: wgpu::Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
            mip_level_count: self.mip_level_count,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.format.into(),
            usage: self.usage.into(),
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        TextureView { view, _texture: texture }
    }
}
