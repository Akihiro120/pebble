use crate::{
    assets::{handle::Handle, storage::Assets, upload::Asset},
    ecs::system::Res,
    wgpu::{backend::WGPUBackend, gpu_context::GpuContext, mipmap::MipmapGenerator, texture_format::TextureFormat},
};

/// Source data for [`GPUTexture`], loaded from a file or supplied as raw
/// bytes. Fields are private — build one via the
/// [`from_file`](Self::from_file)/[`from_data`](Self::from_data)/[`empty`](Self::empty)
/// constructors rather than as a struct literal.
pub struct Texture {
    /// File to decode — `width`/`height` are inferred from the image.
    /// Takes priority over `data` if both are set.
    file: Option<&'static str>,
    /// Width in pixels. Ignored when loading from `file`.
    width: u32,
    /// Height in pixels. Ignored when loading from `file`.
    height: u32,
    /// GPU pixel format to upload as. Defaults to `Rgba8UnormSrgb`.
    format: TextureFormat,
    /// Raw pixel bytes, used when `file` is `None`.
    data: Option<Vec<u8>>,
    /// Whether to generate a full mip chain (via [`MipmapGenerator`]).
    generate_mips: bool,
}

impl Texture {
    /// Load pixel data from a file. Width/height are inferred from the
    /// decoded image.
    pub fn from_file(path: &'static str) -> Self {
        Self {
            file: Some(path),
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8UnormSrgb,
            data: None,
            generate_mips: false,
        }
    }

    /// Supply raw pixel bytes directly, matching `width`/`height`/`format`.
    pub fn from_data(width: u32, height: u32, format: TextureFormat, data: Vec<u8>) -> Self {
        Self {
            file: None,
            width,
            height,
            format,
            data: Some(data),
            generate_mips: false,
        }
    }

    /// Allocate a texture on the GPU with no initial pixel data. Content is
    /// undefined until written via [`GPUTexture::write`].
    pub fn empty(width: u32, height: u32, format: TextureFormat) -> Self {
        Self {
            file: None,
            width,
            height,
            format,
            data: None,
            generate_mips: false,
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
    /// width/height — the resulting texture would have no pixels, almost
    /// certainly an accidental `0` rather than an intentional one.
    fn validate(&self) {
        if self.data.is_some() && (self.width == 0 || self.height == 0) {
            tracing::warn!(
                "Texture::from_data(): width/height is 0 ({}x{}) — did you swap the argument \
                 order, or forget to pass the real dimensions?",
                self.width,
                self.height,
            );
        }
    }

    /// Consume the builder and return the finished [`Texture`] value.
    pub fn build(self) -> Self {
        self.validate();
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Texture>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        self.validate();
        assets.insert(name, self)
    }
}

/// A texture uploaded to the GPU, ready to bind (e.g. via
/// [`BindingInstanceEntry::Texture`](super::instance::BindingInstanceEntry::Texture)).
/// Opaque — bind it into a bind group via
/// [`BindGroupBuilder::texture_2d`](super::buffers::BindGroupBuilder::texture_2d),
/// there's no way to reach the underlying `wgpu::Texture`/`TextureView` from
/// outside this crate.
pub struct GPUTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: TextureFormat,
    ctx: GpuContext,
}

impl GPUTexture {
    /// Overwrites this texture's level-0 pixel data (`pixels` must match the
    /// dimensions/format this texture was uploaded with). Mip levels beyond
    /// 0 are *not* regenerated — if this texture was built `with_mips()`,
    /// they'll go stale relative to the new level-0 data.
    pub fn write(&self, pixels: &[u8]) {
        write_texture_level0(self.ctx.queue(), &self.texture, 0, self.format.into(), self.width, self.height, pixels);
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

/// Overwrites one `origin_z`-indexed layer/face's level-0 pixel data (`0` for
/// a plain [`GPUTexture`], a layer index for [`GPUTextureArray`](super::texture_array::GPUTextureArray),
/// a face index for [`GPUCubemap`](super::cubemap::GPUCubemap)) — the one
/// piece of `write_texture` bookkeeping shared by all three, so a future fix
/// to it (mip handling, row alignment, ...) doesn't need to land in three
/// places independently.
pub(crate) fn write_texture_level0(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    origin_z: u32,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    pixels: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
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

/// Bytes-per-pixel for every regular (non-block-compressed, non-multi-planar,
/// non-depth/stencil) texture format — anything with a well-defined linear
/// CPU-side byte layout, which covers every format [`decode_file`] can
/// actually decode into plus everything reasonable to upload via
/// [`Texture::from_data`]. Block-compressed formats (`Bc*`,
/// `Etc2*`/`Eac*`, `Astc`) need block-aware row/height math this helper
/// doesn't do, multi-planar formats (`NV12`/`P010`) need per-plane byte
/// layouts, and depth/stencil formats aren't meaningful to upload arbitrary
/// pixel bytes into in the first place — all three panic here.
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

/// Keeps the first `channels` of every 4-channel (RGBA) pixel, dropping the rest.
fn take_channels_u8(rgba: &[u8], channels: usize) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| p[..channels].to_vec()).collect()
}

/// Swaps the R and B bytes of every RGBA8 pixel — `image` only decodes to
/// RGB byte order, so this is how `Bgra8*` gets its channels in the order
/// wgpu expects.
fn bgra_swap(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

/// Keeps the first `channels` of every 4-channel `f32` pixel, packed down to
/// half-precision floats.
fn take_channels_f16(rgba32f: &[f32], channels: usize) -> Vec<u8> {
    rgba32f
        .chunks_exact(4)
        .flat_map(|p| p[..channels].iter().flat_map(|c| half::f16::from_f32(*c).to_le_bytes()))
        .collect()
}

/// Keeps the first `channels` of every 4-channel `f32` pixel, as raw `f32` bytes.
fn take_channels_f32(rgba32f: &[f32], channels: usize) -> Vec<u8> {
    rgba32f.chunks_exact(4).flat_map(|p| bytemuck::cast_slice(&p[..channels]).to_vec()).collect()
}

/// Quantizes every 4-channel `f32` pixel (expected in `[0, 1]`) down to
/// 16-bit unsigned normalized integers.
fn rgba32f_to_unorm16(rgba32f: &[f32]) -> Vec<u8> {
    rgba32f
        .iter()
        .flat_map(|c| ((c.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes())
        .collect()
}

/// Decodes an image file into raw pixel bytes matching `format`.
///
/// LDR 8-bit formats (`Rgba8*`, `Bgra8*`, `R8Unorm`, `Rg8Unorm`) decode
/// straight through `to_rgba8()`, keeping/reordering channels as needed.
/// `Rgba16Unorm` decodes through `to_rgba32f()` and quantizes down.
/// Float formats (`R32Float`/`Rg32Float`/`Rgba32Float`, and the 16-bit float
/// variants) decode through `to_rgba32f()` so HDR/EXR sources outside
/// `[0, 1]` survive, then get packed down to the requested channel count and
/// float width.
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

impl Asset<WGPUBackend> for GPUTexture {
    type Source = Texture;
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        source: &Texture,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<Self> {
        // resolve actual pixel data + real dimensions, whether from a file or already-supplied bytes
        let (width, height, data) = if let Some(path) = source.file {
            let (w, h, d) = decode_file(path, source.format.into())?;
            (w, h, Some(d))
        } else if let Some(data) = &source.data {
            (source.width, source.height, Some(data.clone()))
        } else {
            // empty texture — no initial data, content is undefined until written
            (source.width, source.height, None)
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
            format: source.format.into(),
            usage: super::mipmap::texture_usage(mip_count),
            view_formats: &[],
        });

        if let Some(data) = &data {
            // upload level 0 only — fast, synchronous, matches the deferred-mip decision
            backend.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::default(),
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
                1,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some(Self {
            texture,
            view,
            width,
            height,
            format: source.format,
            ctx: GpuContext::from_backend(backend),
        })
    }
}

crate::wgpu::plugin_macros::mipmap_asset_plugin! {
    /// Registers the [`GPUTexture`] asset pipeline (`Assets<Texture>`
    /// → `ProcessedAssets<GPUTexture>`), plus the [`MipmapGenerator`] it depends
    /// on for `generate_mips`. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    TexturePlugin, GPUTexture
}
