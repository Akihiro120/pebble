use crate::{
    assets::{plugin::AssetPlugin, singleton_asset::LazyResourcePlugin, upload::Asset},
    ecs::{plugin::Plugin, system::Res},
    wgpu::{
        backend::WGPUBackend,
        mipmap::MipmapGenerator,
        textures::{bytes_per_pixel, decode_file},
    },
};

pub struct CubemapDescriptor {
    pub size: u32, // cubemaps are square per face
    pub format: wgpu::TextureFormat,
    /// `Some` uploads 6 faces of pixel data up front (wgpu's expected
    /// order: +X, -X, +Y, -Y, +Z, -Z). `None` allocates an empty cubemap
    /// meant to be filled later by rendering into per-face views — e.g. an
    /// environment-map capture pass — in which case [`wgpu_descriptor`](Self::wgpu_descriptor)
    /// adds `RENDER_ATTACHMENT` usage instead of requiring upload data.
    pub faces: Option<[Vec<u8>; 6]>,
    /// File paths for each of the 6 faces (same order as `faces`), decoded
    /// through the same loader used by `GPUTexture`/`GPUTextureArray`.
    pub face_files: Option<[&'static str; 6]>,
    pub generate_mips: bool,
}

impl CubemapDescriptor {
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }

    fn wgpu_descriptor(&self, mip_count: u32, render_target: bool) -> wgpu::TextureDescriptor<'_> {
        wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: if render_target {
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
            },
            view_formats: &[],
        }
    }
}

pub struct GPUCubemap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Asset<WGPUBackend> for GPUCubemap {
    type Source = CubemapDescriptor;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &CubemapDescriptor,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        let faces: Option<[Vec<u8>; 6]> = if let Some(files) = &source.face_files {
            let mut out: [Vec<u8>; 6] = Default::default();
            for (i, path) in files.iter().enumerate() {
                let (w, h, data) = decode_file(path, source.format);
                if w != source.size || h != source.size {
                    tracing::error!(
                        "CubemapSpec: face {i} ('{path}') is {w}x{h}, expected {0}x{0}",
                        source.size
                    );
                    return None;
                }
                out[i] = data;
            }
            Some(out)
        } else {
            source.faces.clone()
        };

        let mip_count = if source.generate_mips {
            (source.size as f32).log2().floor() as u32 + 1
        } else {
            1
        };

        let texture = backend
            .device
            .create_texture(&source.wgpu_descriptor(mip_count, faces.is_none()));

        if let Some(faces) = &faces {
            for (face, data) in faces.iter().enumerate() {
                backend.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: face as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_pixel(source.format) * source.size),
                        rows_per_image: Some(source.size),
                    },
                    wgpu::Extent3d {
                        width: source.size,
                        height: source.size,
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
                    6,
                );
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        Some(Self { texture, view })
    }
}

pub struct CubemapPlugin;
impl Plugin for CubemapPlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_plugin(LazyResourcePlugin::<WGPUBackend, MipmapGenerator>::new());
        app.add_plugin(AssetPlugin::<WGPUBackend, GPUCubemap>::new());
    }
}
