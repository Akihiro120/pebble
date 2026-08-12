use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            mipmap::{MipLevels, MipmapGenerator},
            texture_view::TextureView,
            textures::{bytes_per_pixel, check_texture_dimensions, decode_file, write_texture_mip},
        },
        render::{Backend, gpu_context::GpuContext},
        types::TextureFormat,
    },
};

/// A cubemap texture asset — six square faces of equal size, same
/// construction pattern as [`Texture`](super::textures::Texture). Face
/// order follows wgpu's convention: `+X, -X, +Y, -Y, +Z, -Z`.
pub struct Cubemap {
    size: u32,
    format: TextureFormat,
    faces: Option<[Vec<u8>; 6]>,
    face_files: Option<[&'static str; 6]>,
    mip_levels: MipLevels,
}

impl Cubemap {
    pub fn from_files(size: u32, files: [&'static str; 6]) -> Self {
        Self { size, format: TextureFormat::Rgba8UnormSrgb, faces: None, face_files: Some(files), mip_levels: MipLevels::None }
    }

    pub fn from_data(size: u32, format: TextureFormat, faces: [Vec<u8>; 6]) -> Self {
        Self { size, format, faces: Some(faces), face_files: None, mip_levels: MipLevels::None }
    }

    /// No source data — a render target (e.g. for baking an environment map), or something you'll [`write_face`](GPUCubemap::write_face) yourself.
    pub fn empty(size: u32, format: TextureFormat) -> Self {
        Self { size, format, faces: None, face_files: None, mip_levels: MipLevels::None }
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
        if self.size == 0 && (self.faces.is_some() || self.face_files.is_some()) {
            tracing::warn!(
                "Cubemap::from_data()/from_files(): size is 0 — did you forget to pass the real size?"
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Cubemap>) -> Handle<Cubemap> {
        self.validate();
        assets.insert(name, self)
    }

    /// CPU-side faces — only ever `Some` for a `from_data()` cubemap. See
    /// [`Texture::data`](super::textures::Texture::data).
    pub fn faces(&self) -> Option<&[Vec<u8>; 6]> {
        self.faces.as_ref()
    }

    /// Frees the CPU-side copy. See
    /// [`Texture::release_cpu_data`](super::textures::Texture::release_cpu_data).
    pub fn release_cpu_data(&mut self) {
        self.faces = None;
    }

    fn wgpu_descriptor(&self, mip_count: u32, render_target: bool) -> wgpu::TextureDescriptor<'_> {
        let mut usage = crate::graphics::pipeline::mipmap::texture_usage(mip_count);
        if render_target {
            usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        }

        wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: self.size, height: self.size, depth_or_array_layers: 6 },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format.into(),
            usage,
            view_formats: &[],
        }
    }
}

/// The GPU-resident cubemap an uploaded [`Cubemap`] produces.
pub struct GPUCubemap {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: u32,
    format: TextureFormat,
    ctx: GpuContext,
}

impl GPUCubemap {
    /// Overwrites one mip level of one face with new pixel data.
    pub fn write_face(&self, face: u32, mip_level: u32, pixels: &[u8]) {
        write_texture_mip(self.ctx.queue(), &self.texture, face, mip_level, self.format.into(), self.size, self.size, pixels);
    }

    /// A view into a single face and mip level.
    pub fn get_view(&self, face: u32, mip_level: u32) -> TextureView {
        assert!(face < 6, "GPUCubemap::get_view: face {face} out of range (0..=5)");
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            base_array_layer: face,
            array_layer_count: Some(1),
            ..Default::default()
        });
        TextureView::from_raw(view, self.texture.clone())
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl AssetSource for Cubemap {
    type Processed = GPUCubemap;
}

impl Asset<Backend> for Cubemap {
    type Deps<'a> = Read<'a, MipmapGenerator>;

    fn upload<'a>(&self, backend: &Backend, mipmap_generator: &Read<'a, MipmapGenerator>) -> Option<GPUCubemap> {
        let faces: Option<[Vec<u8>; 6]> = if let Some(files) = &self.face_files {
            let mut out: [Vec<u8>; 6] = Default::default();
            for (i, path) in files.iter().enumerate() {
                let (w, h, data) = decode_file(path, self.format.into())?;
                if w != self.size || h != self.size {
                    tracing::error!(
                        "CubemapSpec: face {i} ('{path}') is {w}x{h}, expected {0}x{0}",
                        self.size
                    );
                    return None;
                }
                out[i] = data;
            }
            Some(out)
        } else {
            self.faces.clone()
        };

        check_texture_dimensions(&backend.device, "GPUCubemap", self.size, self.size);

        let mip_count = crate::graphics::pipeline::mipmap::mip_count(self.size, self.mip_levels);

        let texture = backend.device.create_texture(&self.wgpu_descriptor(mip_count, faces.is_none()));

        if let Some(faces) = &faces {
            for (face, data) in faces.iter().enumerate() {
                backend.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: face as u32 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_pixel(self.format.into()) * self.size),
                        rows_per_image: Some(self.size),
                    },
                    wgpu::Extent3d { width: self.size, height: self.size, depth_or_array_layers: 1 },
                );
            }

            if mip_count > 1 {
                mipmap_generator.generate_mips(backend, &texture, self.format.into(), mip_count, 6);
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        Some(GPUCubemap { texture, view, size: self.size, format: self.format, ctx: GpuContext::from_backend(backend) })
    }
}
