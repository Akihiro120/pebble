use crate::{
    assets::{handle::Handle, storage::Assets, upload::Asset},
    ecs::system::Res,
    wgpu::{
        backend::WGPUBackend,
        gpu_context::GpuContext,
        mipmap::MipmapGenerator,
        texture_format::TextureFormat,
        textures::{bytes_per_pixel, decode_file, write_texture_level0},
    },
};

/// Source data for [`GPUTextureArray`]. Fields are private — build one via
/// the [`from_files`](Self::from_files)/[`from_data`](Self::from_data)/[`empty`](Self::empty)
/// constructors rather than as a struct literal.
pub struct TextureArray {
    /// One file path per layer. Every layer must decode to the same
    /// `width`/`height`.
    files: Option<Vec<&'static str>>,
    /// Width in pixels. Ignored when loading from `files`.
    width: u32,
    /// Height in pixels. Ignored when loading from `files`.
    height: u32,
    /// GPU pixel format to upload as. Defaults to `Rgba8UnormSrgb`.
    format: TextureFormat,
    /// Raw pixel bytes per layer, used when `files` is `None`.
    data: Option<Vec<Vec<u8>>>,
    /// Whether to generate a full mip chain (via [`MipmapGenerator`]).
    generate_mips: bool,
    /// Number of layers to allocate. Only used when both `files` and `data`
    /// are `None` (i.e. [`empty`](Self::empty)); otherwise layer count is
    /// derived from the length of `files`/`data`.
    layer_count: u32,
}

impl TextureArray {
    /// Load one layer per file. Width/height are inferred from the first
    /// file and every subsequent layer must match.
    pub fn from_files(files: Vec<&'static str>) -> Self {
        Self {
            files: Some(files),
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8UnormSrgb,
            data: None,
            generate_mips: false,
            layer_count: 0,
        }
    }

    /// Supply raw pixel bytes per layer directly, matching `width`/`height`/`format`.
    pub fn from_data(width: u32, height: u32, format: TextureFormat, layers: Vec<Vec<u8>>) -> Self {
        Self {
            files: None,
            width,
            height,
            format,
            data: Some(layers),
            generate_mips: false,
            layer_count: 0,
        }
    }

    /// Allocate an empty texture array on the GPU with no initial pixel data.
    /// Content is undefined until written via [`GPUTextureArray::write_layer`].
    pub fn empty(width: u32, height: u32, format: TextureFormat, layer_count: u32) -> Self {
        Self {
            files: None,
            width,
            height,
            format,
            data: None,
            generate_mips: false,
            layer_count,
        }
    }

    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_mips(mut self) -> Self {
        self.generate_mips = true;
        self
    }

    /// Logs a WARN if [`from_data`](Self::from_data) was given a zero
    /// width/height — same rationale as the equivalent check on
    /// [`Texture`](super::textures::Texture).
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

    /// Consume the builder and return the finished [`TextureArray`] value.
    pub fn build(self) -> Self {
        self.validate();
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<TextureArray>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        self.validate();
        assets.insert(name, self)
    }
}

/// A 2D texture array uploaded to the GPU, ready to bind (e.g. via
/// [`BindingInstanceEntry::TextureArray`](super::instance::BindingInstanceEntry::TextureArray)).
/// Opaque — bind it via
/// [`BindGroupBuilder::texture_array`](super::buffers::BindGroupBuilder::texture_array).
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
    /// Overwrites one layer's level-0 pixel data. See
    /// [`GPUTexture::write`](super::textures::GPUTexture::write) for the
    /// same caveat about mip levels not being regenerated.
    pub fn write_layer(&self, layer: u32, pixels: &[u8]) {
        write_texture_level0(self.ctx.queue(), &self.texture, layer, self.format.into(), self.width, self.height, pixels);
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

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl Asset<WGPUBackend> for GPUTextureArray {
    type Source = TextureArray;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &TextureArray,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        let (width, height, layer_count, layers): (u32, u32, u32, Option<Vec<Vec<u8>>>) =
            if let Some(files) = &source.files {
                let mut width = source.width;
                let mut height = source.height;
                let mut layers = Vec::with_capacity(files.len());
                for (i, path) in files.iter().enumerate() {
                    let (w, h, data) = decode_file(path, source.format.into())?;
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
            } else if let Some(data) = &source.data {
                let count = data.len() as u32;
                (source.width, source.height, count, Some(data.clone()))
            } else {
                // empty array — no initial data, content is undefined until written
                (source.width, source.height, source.layer_count, None)
            };

        if layer_count == 0 {
            tracing::error!("TextureArraySpec resolved to zero layers");
            return None;
        }

        let mip_count = super::mipmap::mip_count(width.max(height), source.generate_mips);

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: layer_count,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: source.format.into(),
            usage: super::mipmap::texture_usage(mip_count),
            view_formats: &[],
        });

        if let Some(layers) = &layers {
            for (layer, data) in layers.iter().enumerate() {
                backend.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_pixel(source.format.into()) * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            }

            if mip_count > 1 {
                mipmap_generator.generate_mips(
                    &backend.device,
                    &backend.queue,
                    &texture,
                    source.format.into(),
                    mip_count,
                    layer_count,
                );
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Some(Self {
            texture,
            view,
            layer_count,
            width,
            height,
            format: source.format,
            ctx: GpuContext::from_backend(backend),
        })
    }
}

crate::wgpu::plugin_macros::mipmap_asset_plugin! {
    /// Registers the [`GPUTextureArray`] asset pipeline
    /// (`Assets<TextureArray>` → `ProcessedAssets<GPUTextureArray>`),
    /// plus the [`MipmapGenerator`] it depends on for `generate_mips`. Included
    /// by [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    TextureArrayPlugin, GPUTextureArray
}
