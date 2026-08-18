/// A GPU texture view — what a [`ColorTarget`](crate::graphics::render::targets::ColorTarget)/
/// [`DepthTarget`](crate::graphics::render::targets::DepthTarget) or a bind
/// group entry actually points at. Build one via a texture's `get_view()` —
/// for a standalone render target, `Texture::empty(w, h, format)`
/// (`.with_sample_count(...)`/`.with_extra_usage(...)` for MSAA or extra
/// usage flags) build_asset'd like any other [`Texture`](super::textures::Texture), then
/// `GPUTexture::get_view(mip)` once it's uploaded.
#[derive(Clone)]
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
