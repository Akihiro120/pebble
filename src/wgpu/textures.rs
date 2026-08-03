use crate::{
    assets::upload::Asset,
    ecs::system::Res,
    wgpu::{backend::WGPUBackend, mipmap::MipmapGenerator},
};

/// Source data for [`GPUTexture`], loaded from a file or supplied as raw
/// bytes. Prefer the [`from_file`](Self::from_file)/[`from_data`](Self::from_data)
/// constructors over setting fields by hand.
pub struct TextureDescriptor {
    /// File to decode — `width`/`height` are inferred from the image.
    /// Takes priority over `data` if both are set.
    pub file: Option<&'static str>,
    /// Width in pixels. Ignored when loading from `file`.
    pub width: u32,
    /// Height in pixels. Ignored when loading from `file`.
    pub height: u32,
    /// GPU pixel format to upload as. Defaults to `Rgba8UnormSrgb`.
    pub format: wgpu::TextureFormat,
    /// Raw pixel bytes, used when `file` is `None`.
    pub data: Option<Vec<u8>>,
    /// Whether to generate a full mip chain (via [`MipmapGenerator`]).
    pub generate_mips: bool,
}

impl TextureDescriptor {
    /// Load pixel data from a file. Width/height are inferred from the
    /// decoded image.
    pub fn from_file(path: &'static str) -> Self {
        Self {
            file: Some(path),
            width: 0,
            height: 0,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            data: None,
            generate_mips: false,
        }
    }

    /// Supply raw pixel bytes directly, matching `width`/`height`/`format`.
    pub fn from_data(width: u32, height: u32, format: wgpu::TextureFormat, data: Vec<u8>) -> Self {
        Self {
            file: None,
            width,
            height,
            format,
            data: Some(data),
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

/// A texture uploaded to the GPU, ready to bind (e.g. via
/// [`BindingInstanceEntry::Texture`](super::instance::BindingInstanceEntry::Texture)).
pub struct GPUTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

/// Bytes-per-pixel for the pixel formats this loader knows how to produce.
pub(crate) fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        wgpu::TextureFormat::Rgba32Float => 16,
        other => panic!("unsupported texture format for GPUTexture: {other:?}"),
    }
}

/// Decodes an image file into raw pixel bytes matching `format`.
///
/// LDR formats (Rgba8*) decode straight to 8-bit RGBA. HDR/EXR sources (and
/// any request for a float format) decode through `to_rgba32f()` so that
/// values outside `[0, 1]` survive, then get packed down to the requested
/// float width.
pub(crate) fn decode_file(path: &str, format: wgpu::TextureFormat) -> Option<(u32, u32, Vec<u8>)> {
    let img = match image::open(path) {
        Ok(img) => img,
        Err(e) => {
            tracing::error!("failed to load texture '{path}': {e}");
            return None;
        }
    };

    Some(match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            (w, h, img.into_raw())
        }
        wgpu::TextureFormat::Rgba32Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            let bytes = bytemuck::cast_slice(img.into_raw().as_slice()).to_vec();
            (w, h, bytes)
        }
        wgpu::TextureFormat::Rgba16Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            let bytes = img
                .into_raw()
                .into_iter()
                .flat_map(|c| half::f16::from_f32(c).to_le_bytes())
                .collect();
            (w, h, bytes)
        }
        other => panic!("unsupported texture format for GPUTexture: {other:?}"),
    })
}

impl Asset<WGPUBackend> for GPUTexture {
    type Source = TextureDescriptor;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &TextureDescriptor,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        // resolve actual pixel data + real dimensions, whether from a file or already-supplied bytes
        let (width, height, data) = if let Some(path) = source.file {
            decode_file(path, source.format)?
        } else if let Some(data) = &source.data {
            (source.width, source.height, data.clone())
        } else {
            tracing::error!("TextureSpec has neither `file` nor `data` set");
            return None;
        };

        let mip_count = super::mipmap::mip_count(width.max(height), source.generate_mips);

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count, // room allocated for all levels now
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: source.format,
            usage: super::mipmap::texture_usage(mip_count),
            view_formats: &[],
        });

        // upload level 0 only — fast, synchronous, matches the deferred-mip decision
        backend.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::default(),
                aspect: wgpu::TextureAspect::All,
            },
            &data,
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

        if mip_count > 1 {
            mipmap_generator.generate_mips(
                &backend.device,
                &backend.queue,
                &texture,
                source.format,
                mip_count,
                1,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some(Self { texture, view })
    }
}

crate::wgpu::plugin_macros::mipmap_asset_plugin! {
    /// Registers the [`GPUTexture`] asset pipeline (`Assets<TextureDescriptor>`
    /// → `ProcessedAssets<GPUTexture>`), plus the [`MipmapGenerator`] it depends
    /// on for `generate_mips`. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    TexturePlugin, GPUTexture
}
