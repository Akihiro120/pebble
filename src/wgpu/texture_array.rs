use crate::{
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::{plugin::Plugin, system::Res},
    wgpu::{
        backend::WGPUBackend,
        mipmap::MipmapGenerator,
        textures::{bytes_per_pixel, decode_file},
    },
};

pub struct TextureArrayDescriptor {
    /// One file path per layer. Every layer must decode to the same
    /// `width`/`height`.
    pub files: Option<Vec<&'static str>>,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// Raw pixel bytes per layer, used when `files` is `None`.
    pub data: Option<Vec<Vec<u8>>>,
    pub generate_mips: bool,
}

impl TextureArrayDescriptor {
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
                let (w, h, data) = decode_file(path, source.format);
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
                depth_or_array_layers: layer_count,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: source.format,
            usage: if mip_count > 1 {
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
            },
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

#[derive(Default)]
pub struct TextureArrayPlugin;
impl TextureArrayPlugin {
    pub fn new() -> Self {
        Self
    }
}
impl Plugin for TextureArrayPlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_plugin(crate::assets::singleton_asset::LazyResourcePlugin::<
            WGPUBackend,
            MipmapGenerator,
        >::new());
        app.add_plugin(AssetPlugin::<WGPUBackend, GPUTextureArray>::new());
    }
}
