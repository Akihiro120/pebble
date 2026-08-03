use crate::{
    assets::upload::Asset,
    ecs::system::Res,
    wgpu::{
        backend::WGPUBackend,
        mipmap::MipmapGenerator,
        textures::{bytes_per_pixel, decode_file},
    },
};

/// Source data for [`GPUTextureArray`]. Prefer the
/// [`from_files`](Self::from_files)/[`from_data`](Self::from_data)
/// constructors over setting fields by hand.
pub struct TextureArrayDescriptor {
    /// One file path per layer. Every layer must decode to the same
    /// `width`/`height`.
    pub files: Option<Vec<&'static str>>,
    /// Width in pixels. Ignored when loading from `files`.
    pub width: u32,
    /// Height in pixels. Ignored when loading from `files`.
    pub height: u32,
    /// GPU pixel format to upload as. Defaults to `Rgba8UnormSrgb`.
    pub format: wgpu::TextureFormat,
    /// Raw pixel bytes per layer, used when `files` is `None`.
    pub data: Option<Vec<Vec<u8>>>,
    /// Whether to generate a full mip chain (via [`MipmapGenerator`]).
    pub generate_mips: bool,
}

impl TextureArrayDescriptor {
    /// Load one layer per file. Width/height are inferred from the first
    /// file and every subsequent layer must match.
    pub fn from_files(files: Vec<&'static str>) -> Self {
        Self {
            files: Some(files),
            width: 0,
            height: 0,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            data: None,
            generate_mips: false,
        }
    }

    /// Supply raw pixel bytes per layer directly, matching `width`/`height`/`format`.
    pub fn from_data(width: u32, height: u32, format: wgpu::TextureFormat, layers: Vec<Vec<u8>>) -> Self {
        Self {
            files: None,
            width,
            height,
            format,
            data: Some(layers),
            generate_mips: false,
        }
    }

    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }

    pub fn with_mips(mut self) -> Self {
        self.generate_mips = true;
        self
    }
}

/// A 2D texture array uploaded to the GPU, ready to bind (e.g. via
/// [`BindingInstanceEntry::TextureArray`](super::instance::BindingInstanceEntry::TextureArray)).
pub struct GPUTextureArray {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub layer_count: u32,
}

impl Asset<WGPUBackend> for GPUTextureArray {
    type Source = TextureArrayDescriptor;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &TextureArrayDescriptor,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        let (width, height, layers): (u32, u32, Vec<Vec<u8>>) = if let Some(files) = &source.files {
            let mut width = source.width;
            let mut height = source.height;
            let mut layers = Vec::with_capacity(files.len());
            for (i, path) in files.iter().enumerate() {
                let (w, h, data) = decode_file(path, source.format)?;
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
            (width, height, layers)
        } else if let Some(data) = &source.data {
            (source.width, source.height, data.clone())
        } else {
            tracing::error!("TextureArraySpec has neither `files` nor `data` set");
            return None;
        };

        if layers.is_empty() {
            tracing::error!("TextureArraySpec resolved to zero layers");
            return None;
        }
        let layer_count = layers.len() as u32;

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
            format: source.format,
            usage: super::mipmap::texture_usage(mip_count),
            view_formats: &[],
        });

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
                    bytes_per_row: Some(bytes_per_pixel(source.format) * width),
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
                source.format,
                mip_count,
                layer_count,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Some(Self {
            texture,
            view,
            layer_count,
        })
    }
}

crate::wgpu::plugin_macros::mipmap_asset_plugin! {
    /// Registers the [`GPUTextureArray`] asset pipeline
    /// (`Assets<TextureArrayDescriptor>` → `ProcessedAssets<GPUTextureArray>`),
    /// plus the [`MipmapGenerator`] it depends on for `generate_mips`. Included
    /// by [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    TextureArrayPlugin, GPUTextureArray
}
