use crate::{
    assets::{plugin::AssetPlugin, singleton_asset::LazyResourcePlugin, upload::Asset},
    ecs::{plugin::Plugin, system::Res},
    wgpu::{backend::WGPUBackend, mipmap::MipmapGenerator},
};

pub struct TextureSpec {
    pub file: Option<&'static str>,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub data: Option<Vec<u8>>,
    pub generate_mips: bool,
}

impl TextureSpec {
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}

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
pub(crate) fn decode_file(path: &str, format: wgpu::TextureFormat) -> (u32, u32, Vec<u8>) {
    let img = image::open(path).unwrap_or_else(|e| panic!("failed to load texture '{path}': {e}"));

    match format {
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
    }
}

impl Asset<WGPUBackend> for GPUTexture {
    type Source = TextureSpec;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &TextureSpec,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        // resolve actual pixel data + real dimensions, whether from a file or already-supplied bytes
        let (width, height, data) = if let Some(path) = source.file {
            decode_file(path, source.format)
        } else if let Some(data) = &source.data {
            (source.width, source.height, data.clone())
        } else {
            tracing::error!("TextureSpec has neither `file` nor `data` set");
            return None;
        };

        let mip_count = if source.generate_mips {
            (width.max(height) as f32).log2().floor() as u32 + 1
        } else {
            1
        };

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
            usage: if mip_count > 1 {
                // mips beyond level 0 are rendered into on the GPU, so the
                // texture needs to double as a render attachment
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
            },
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

pub struct TexturePlugin;
impl Plugin for TexturePlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_plugin(LazyResourcePlugin::<WGPUBackend, MipmapGenerator>::new());
        app.add_plugin(AssetPlugin::<WGPUBackend, GPUTexture>::new());
    }
}
