use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{mipmap::{MipLevels, MipmapGenerator}, texture_view::TextureView},
        render::{Backend, gpu_context::GpuContext},
        types::TextureFormat,
    },
};

pub struct Texture {
    file: Option<&'static str>,
    width: u32,
    height: u32,
    format: TextureFormat,
    data: Option<Vec<u8>>,
    mip_levels: MipLevels,
}

impl Texture {
    pub fn from_file(path: &'static str) -> Self {
        Self { file: Some(path), width: 0, height: 0, format: TextureFormat::Rgba8UnormSrgb, data: None, mip_levels: MipLevels::None }
    }

    pub fn from_data(width: u32, height: u32, format: TextureFormat, data: Vec<u8>) -> Self {
        Self { file: None, width, height, format, data: Some(data), mip_levels: MipLevels::None }
    }

    pub fn empty(width: u32, height: u32, format: TextureFormat) -> Self {
        Self { file: None, width, height, format, data: None, mip_levels: MipLevels::None }
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
                "Texture::from_data(): width/height is 0 ({}x{}) — did you swap the \
                 argument order, or forget to pass the real dimensions?",
                self.width,
                self.height,
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Texture>) -> Handle<Texture> {
        self.validate();
        assets.insert(name, self)
    }
}

pub struct GPUTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: TextureFormat,
    ctx: GpuContext,
}

impl GPUTexture {
    pub fn write(&self, mip_level: u32, pixels: &[u8]) {
        write_texture_mip(self.ctx.queue(), &self.texture, 0, mip_level, self.format.into(), self.width, self.height, pixels);
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn get_view(&self, mip_level: u32) -> TextureView {
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            ..Default::default()
        });
        TextureView::from_raw(view, self.texture.clone())
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub(crate) fn write_texture_mip(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    origin_z: u32,
    mip_level: u32,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    pixels: &[u8],
) {
    let width = (width >> mip_level).max(1);
    let height = (height >> mip_level).max(1);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level,
            origin: wgpu::Origin3d { x: 0, y: 0, z: origin_z },
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_pixel(format) * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
}

pub(crate) fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
    use wgpu::TextureFormat as F;
    match format {
        F::R8Unorm | F::R8Snorm | F::R8Uint | F::R8Sint => 1,
        F::R16Uint | F::R16Sint | F::R16Unorm | F::R16Snorm | F::R16Float | F::Rg8Unorm | F::Rg8Snorm
        | F::Rg8Uint | F::Rg8Sint => 2,
        F::R32Uint | F::R32Sint | F::R32Float | F::Rg16Uint | F::Rg16Sint | F::Rg16Unorm | F::Rg16Snorm
        | F::Rg16Float | F::Rgba8Unorm | F::Rgba8UnormSrgb | F::Rgba8Snorm | F::Rgba8Uint | F::Rgba8Sint
        | F::Bgra8Unorm | F::Bgra8UnormSrgb | F::Rgb10a2Uint | F::Rgb10a2Unorm | F::Rg11b10Ufloat
        | F::Rgb9e5Ufloat => 4,
        F::R64Uint | F::Rg32Uint | F::Rg32Sint | F::Rg32Float | F::Rgba16Uint | F::Rgba16Sint
        | F::Rgba16Unorm | F::Rgba16Snorm | F::Rgba16Float => 8,
        F::Rgba32Uint | F::Rgba32Sint | F::Rgba32Float => 16,
        other => panic!(
            "unsupported texture format for GPUTexture: {other:?} — block-compressed, \
             multi-planar, and depth/stencil formats have no linear CPU-side pixel layout \
             this helper can compute"
        ),
    }
}

pub(crate) fn check_texture_dimensions(device: &wgpu::Device, what: &str, width: u32, height: u32) {
    let max = device.limits().max_texture_dimension_2d;
    if width > max || height > max {
        panic!("{what}: {width}x{height} exceeds this device's max_texture_dimension_2d ({max})");
    }
}

pub(crate) fn check_texture_array_layers(device: &wgpu::Device, what: &str, layer_count: u32) {
    let max = device.limits().max_texture_array_layers;
    if layer_count > max {
        panic!("{what}: {layer_count} layers exceeds this device's max_texture_array_layers ({max})");
    }
}

fn take_channels_u8(rgba: &[u8], channels: usize) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| p[..channels].to_vec()).collect()
}

fn bgra_swap(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

fn take_channels_f16(rgba32f: &[f32], channels: usize) -> Vec<u8> {
    rgba32f
        .chunks_exact(4)
        .flat_map(|p| p[..channels].iter().flat_map(|c| half::f16::from_f32(*c).to_le_bytes()))
        .collect()
}

fn take_channels_f32(rgba32f: &[f32], channels: usize) -> Vec<u8> {
    rgba32f.chunks_exact(4).flat_map(|p| bytemuck::cast_slice(&p[..channels]).to_vec()).collect()
}

fn rgba32f_to_unorm16(rgba32f: &[f32]) -> Vec<u8> {
    rgba32f
        .iter()
        .flat_map(|c| ((c.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes())
        .collect()
}

pub(crate) fn decode_file(path: &str, format: wgpu::TextureFormat) -> Option<(u32, u32, Vec<u8>)> {
    use wgpu::TextureFormat as F;

    let img = match image::open(path) {
        Ok(img) => img,
        Err(e) => {
            tracing::error!("failed to load texture '{path}': {e}");
            return None;
        }
    };

    Some(match format {
        F::Rgba8Unorm | F::Rgba8UnormSrgb => {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            (w, h, img.into_raw())
        }
        F::Bgra8Unorm | F::Bgra8UnormSrgb => {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            (w, h, bgra_swap(&img.into_raw()))
        }
        F::R8Unorm => {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            (w, h, take_channels_u8(&img.into_raw(), 1))
        }
        F::Rg8Unorm => {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            (w, h, take_channels_u8(&img.into_raw(), 2))
        }
        F::Rgba16Unorm => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, rgba32f_to_unorm16(img.into_raw().as_slice()))
        }
        F::Rgba32Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            let bytes = bytemuck::cast_slice(img.into_raw().as_slice()).to_vec();
            (w, h, bytes)
        }
        F::Rg32Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, take_channels_f32(img.into_raw().as_slice(), 2))
        }
        F::R32Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, take_channels_f32(img.into_raw().as_slice(), 1))
        }
        F::Rgba16Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, take_channels_f16(img.into_raw().as_slice(), 4))
        }
        F::Rg16Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, take_channels_f16(img.into_raw().as_slice(), 2))
        }
        F::R16Float => {
            let img = img.to_rgba32f();
            let (w, h) = img.dimensions();
            (w, h, take_channels_f16(img.into_raw().as_slice(), 1))
        }
        other => panic!(
            "unsupported texture format for GPUTexture: {other:?} — file decoding covers the \
             regular 8/16/32-bit unorm and float formats; block-compressed and multi-planar \
             formats aren't decodable from an ordinary image file this way"
        ),
    })
}

impl AssetSource for Texture {
    type Processed = GPUTexture;
}

impl Asset<Backend> for Texture {
    type Deps<'a> = Read<'a, MipmapGenerator>;

    fn upload<'a>(&self, backend: &Backend, mipmap_generator: &Read<'a, MipmapGenerator>) -> Option<GPUTexture> {
        let (width, height, data) = if let Some(path) = self.file {
            let (w, h, d) = decode_file(path, self.format.into())?;
            (w, h, Some(d))
        } else if let Some(data) = &self.data {
            (self.width, self.height, Some(data.clone()))
        } else {
            (self.width, self.height, None)
        };

        check_texture_dimensions(&backend.device, "GPUTexture", width, height);

        let mip_count = crate::graphics::pipeline::mipmap::mip_count(width.max(height), self.mip_levels);
        let usage = crate::graphics::pipeline::mipmap::texture_usage_for(mip_count, data.is_some());

        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format.into(),
            usage,
            view_formats: &[],
        });

        if let Some(data) = &data {
            write_texture_mip(&backend.queue, &texture, 0, 0, self.format.into(), width, height, data);

            if mip_count > 1 {
                mipmap_generator.generate_mips(backend, &texture, self.format.into(), mip_count, 1);
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some(GPUTexture { texture, view, width, height, format: self.format, ctx: GpuContext::from_backend(backend) })
    }
}
