use crate::graphics::{pipeline::textures::check_texture_dimensions, render::Backend, types::{TextureFormat, flags::TextureUsages}};

/// A GPU texture view — what a [`ColorTarget`](crate::graphics::render::targets::ColorTarget)/
/// [`DepthTarget`](crate::graphics::render::targets::DepthTarget) or a bind group entry actually
/// points at. Build one via a texture's `get_view()`, or [`RenderTargetTextureBuilder`] for a
/// standalone render target.
pub struct TextureView {
    view: wgpu::TextureView,
    _texture: wgpu::Texture,
}

impl TextureView {
    pub(crate) fn raw(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub(crate) fn from_raw(view: wgpu::TextureView, texture: wgpu::Texture) -> Self {
        Self { view, _texture: texture }
    }
}

/// Builds a standalone [`TextureView`] for use as a render target — e.g. a
/// post-processing buffer or shadow map, not backed by any [`Texture`](super::textures::Texture) asset.
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

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_usage(mut self, usage: TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_mip_level_count(mut self, count: u32) -> Self {
        self.mip_level_count = count;
        self
    }

    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    pub fn build(self, backend: &Backend) -> TextureView {
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
