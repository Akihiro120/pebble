use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            mipmap::{MipLevels, MipmapGenerator},
            texture_view::TextureView,
            textures::{bytes_per_pixel, check_texture_array_layers, check_texture_dimensions, decode_file},
        },
        render::{Backend, gpu_context::GpuContext},
        types::TextureFormat,
    },
};

/// A 2D array texture asset — same construction pattern as [`Texture`](super::textures::Texture),
/// but each layer's pixels come from a separate file/buffer. All layers must
/// share the same dimensions.
pub struct TextureArray {
    files: Option<Vec<&'static str>>,
    width: u32,
    height: u32,
    format: TextureFormat,
    data: Option<Vec<Vec<u8>>>,
    layer_count: u32,
    mip_levels: MipLevels,
}

impl TextureArray {
    pub fn from_files(files: Vec<&'static str>) -> Self {
        Self { files: Some(files), width: 0, height: 0, format: TextureFormat::Rgba8UnormSrgb, data: None, layer_count: 0, mip_levels: MipLevels::None }
    }

    pub fn from_data(width: u32, height: u32, format: TextureFormat, layers: Vec<Vec<u8>>) -> Self {
        Self { files: None, width, height, format, data: Some(layers), layer_count: 0, mip_levels: MipLevels::None }
    }

    /// No source data — a render target, or something you'll [`write_layer`](GPUTextureArray::write_layer) yourself.
    pub fn empty(width: u32, height: u32, format: TextureFormat, layer_count: u32) -> Self {
        Self { files: None, width, height, format, data: None, layer_count, mip_levels: MipLevels::None }
    }

    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_mips(mut self) -> Self {
        self.mip_levels = MipLevels::Full;
        self
    }

    pub fn with_mip_count(mut self, count: u32) -> Self {
        self.mip_levels = MipLevels::Fixed(count);
        self
    }

    fn validate(&self) {
        if self.data.is_some() && (self.width == 0 || self.height == 0) {
            tracing::warn!(
                "TextureArray::from_data(): width/height is 0 ({}x{}) — did you swap the \
                 argument order, or forget to pass the real dimensions?",
                self.width,
                self.height,
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<TextureArray>) -> Handle<TextureArray> {
        self.validate();
        assets.insert(name, self)
    }

    /// CPU-side layers — only ever `Some` for a `from_data()` array. See
    /// [`Texture::data`](super::textures::Texture::data).
    pub fn data(&self) -> Option<&[Vec<u8>]> {
        self.data.as_deref()
    }

    /// Frees the CPU-side copy. See
    /// [`Texture::release_cpu_data`](super::textures::Texture::release_cpu_data).
    pub fn release_cpu_data(&mut self) {
        self.data = None;
    }
}

/// The GPU-resident array texture an uploaded [`TextureArray`] produces.
pub struct GPUTextureArray {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    layer_count: u32,
    width: u32,
    height: u32,
    format: TextureFormat,
    ctx: GpuContext,
}

impl GPUTextureArray {
    /// Overwrites one mip level of one layer with new pixel data.
    pub fn write_layer(&self, layer: u32, mip_level: u32, pixels: &[u8]) {
        crate::graphics::pipeline::textures::write_texture_mip(
            self.ctx.queue(),
            &self.texture,
            layer,
            mip_level,
            self.format.into(),
            self.width,
            self.height,
            pixels,
        );
    }

    pub fn layer_count(&self) -> u32 {
        self.layer_count
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// A view into a single layer and mip level.
    pub fn get_view(&self, layer: u32, mip_level: u32) -> TextureView {
        assert!(layer < self.layer_count, "GPUTextureArray::get_view: layer {layer} out of range (0..{})", self.layer_count);
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        });
        TextureView::from_raw(view, self.texture.clone())
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl AssetSource for TextureArray {
    type Processed = GPUTextureArray;
}

impl Asset<Backend> for TextureArray {
    type Deps<'a> = Read<'a, MipmapGenerator>;

    fn upload<'a>(&self, backend: &Backend, mipmap_generator: &Read<'a, MipmapGenerator>) -> Option<GPUTextureArray> {
        let (width, height, layer_count, layers): (u32, u32, u32, Option<Vec<Vec<u8>>>) =
            if let Some(files) = &self.files {
                let mut width = self.width;
                let mut height = self.height;
                let mut layers = Vec::with_capacity(files.len());
                for (i, path) in files.iter().enumerate() {
                    let (w, h, data) = decode_file(path, self.format.into())?;
                    if i == 0 {
                        width = w;
                        height = h;
                    } else if w != width || h != height {
                        tracing::error!(
                            "TextureArraySpec: layer {i} ('{path}') is {w}x{h}, expected {width}x{height}"
                        );
                        return None;
                    }
                    layers.push(data);
                }
                let count = layers.len() as u32;
                (width, height, count, Some(layers))
            } else if let Some(data) = &self.data {
                let count = data.len() as u32;
                (self.width, self.height, count, Some(data.clone()))
            } else {
                (self.width, self.height, self.layer_count, None)
            };

        if layer_count == 0 {
            tracing::error!("TextureArraySpec resolved to zero layers");
            return None;
        }

        check_texture_dimensions(&backend.device, "GPUTextureArray", width, height);
        check_texture_array_layers(&backend.device, "GPUTextureArray", layer_count);

        let mip_count = crate::graphics::pipeline::mipmap::mip_count(width.max(height), self.mip_levels);
        let usage = crate::graphics::pipeline::mipmap::texture_usage_for(mip_count, layers.is_some());

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: layer_count },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format.into(),
            usage,
            view_formats: &[],
        });

        if let Some(layers) = &layers {
            for (layer, data) in layers.iter().enumerate() {
                backend.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_pixel(self.format.into()) * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
            }

            if mip_count > 1 {
                mipmap_generator.generate_mips(backend, &texture, self.format.into(), mip_count, layer_count);
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Some(GPUTextureArray { texture, view, layer_count, width, height, format: self.format, ctx: GpuContext::from_backend(backend) })
    }
}
