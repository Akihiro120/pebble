use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::system::Res,
    wgpu::{
        backend::WGPUBackend,
        flags::TextureUsages,
        gpu_context::GpuContext,
        mipmap::MipmapGenerator,
        texture_format::TextureFormat,
        textures::{bytes_per_pixel, check_texture_dimensions, decode_file, write_texture_level0},
    },
};

/// Source data for [`GPUCubemap`]. Fields are private — the only way to
/// construct one is [`CubemapBuilder`]: `CubemapBuilder::from_files(...).build()`.
pub struct Cubemap {
    /// Edge length in pixels — cubemap faces are always square.
    size: u32,
    /// GPU pixel format to upload as. Defaults to `Rgba8UnormSrgb`.
    format: TextureFormat,
    /// `Some` uploads 6 faces of pixel data up front (wgpu's expected
    /// order: +X, -X, +Y, -Y, +Z, -Z). `None` allocates an empty cubemap
    /// meant to be filled later by rendering into per-face views — e.g. an
    /// environment-map capture pass — in which case [`wgpu_descriptor`](Self::wgpu_descriptor)
    /// adds `RENDER_ATTACHMENT` usage instead of requiring upload data.
    faces: Option<[Vec<u8>; 6]>,
    /// File paths for each of the 6 faces (same order as `faces`), decoded
    /// through the same loader used by `GPUTexture`/`GPUTextureArray`.
    face_files: Option<[&'static str; 6]>,
    /// Whether to generate a full mip chain (via [`MipmapGenerator`]). Only
    /// applies when uploading pixel data (`faces`/`face_files` set) —
    /// meaningless for an [`empty`](CubemapBuilder::empty) render-target cubemap.
    generate_mips: bool,
}

impl Cubemap {
    /// `render_target` is set for an empty capture-target cubemap (see
    /// [`empty`](CubemapBuilder::empty)), rendered into directly. Separately
    /// from that, `mip_count > 1` also needs `RENDER_ATTACHMENT` — mips
    /// beyond level 0 are rendered into by [`MipmapGenerator::generate_mips`](super::mipmap::MipmapGenerator::generate_mips)
    /// regardless of whether the base texture is a capture target or one
    /// uploaded from real face data, so the two conditions are OR'd rather
    /// than `render_target` alone deciding the usage.
    fn wgpu_descriptor(&self, mip_count: u32, render_target: bool) -> wgpu::TextureDescriptor<'_> {
        let mut usage = super::mipmap::texture_usage(mip_count);
        if render_target {
            usage |= TextureUsages::RENDER_ATTACHMENT.into();
        }

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
            format: self.format.into(),
            usage,
            view_formats: &[],
        }
    }
}

/// Builds a [`Cubemap`] — load 6 faces from files, supply raw bytes for each
/// face directly, or allocate an empty cubemap with no initial data. Start
/// from [`from_files`](Self::from_files)/[`from_faces`](Self::from_faces)/[`empty`](Self::empty),
/// chain the setters below, then finish with
/// [`build`](Self::build)/[`build_asset`](Self::build_asset).
pub struct CubemapBuilder {
    size: u32,
    format: TextureFormat,
    faces: Option<[Vec<u8>; 6]>,
    face_files: Option<[&'static str; 6]>,
    generate_mips: bool,
}

impl CubemapBuilder {
    /// Load 6 faces from files (+X, -X, +Y, -Y, +Z, -Z). Size is inferred from the first face.
    pub fn from_files(size: u32, files: [&'static str; 6]) -> Self {
        Self {
            size,
            format: TextureFormat::Rgba8UnormSrgb,
            faces: None,
            face_files: Some(files),
            generate_mips: false,
        }
    }

    /// Supply raw pixel bytes for each face (+X, -X, +Y, -Y, +Z, -Z).
    pub fn from_faces(size: u32, format: TextureFormat, faces: [Vec<u8>; 6]) -> Self {
        Self {
            size,
            format,
            faces: Some(faces),
            face_files: None,
            generate_mips: false,
        }
    }

    /// Allocate an empty cubemap for use as a render target (e.g. environment capture).
    pub fn empty(size: u32, format: TextureFormat) -> Self {
        Self {
            size,
            format,
            faces: None,
            face_files: None,
            generate_mips: false,
        }
    }

    /// Override the format set by whichever constructor was used (all
    /// three default to or take `format` directly — this exists for the
    /// builder-chain case, e.g. `CubemapBuilder::empty(size, format).with_mips()`
    /// followed later by a format change, without re-specifying `size`).
    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = format;
        self
    }

    /// Enable full mip chain generation.
    pub fn with_mips(mut self) -> Self {
        self.generate_mips = true;
        self
    }

    /// Logs a WARN for `with_mips()` called on an [`empty`](Self::empty)
    /// cubemap — per `generate_mips`'s own doc, mip generation only applies
    /// when uploading real face data, so this combination is a no-op.
    fn validate(&self) {
        if self.generate_mips && self.faces.is_none() && self.face_files.is_none() {
            tracing::warn!(
                "CubemapBuilder: with_mips() has no effect on an empty() render-target cubemap \
                 — mip generation only applies when faces/face_files supply pixel data"
            );
        }
    }

    /// Consume the builder and return the finished [`Cubemap`] value.
    pub fn build(self) -> Cubemap {
        self.validate();
        Cubemap {
            size: self.size,
            format: self.format,
            faces: self.faces,
            face_files: self.face_files,
            generate_mips: self.generate_mips,
        }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Cubemap>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Cubemap>) -> Handle<Cubemap> {
        let cubemap = self.build();
        assets.insert(name, cubemap)
    }
}

/// A cubemap uploaded to the GPU, ready to bind (e.g. via
/// [`BindingInstanceEntry::Cubemap`](super::instance::BindingInstanceEntry::Cubemap)).
/// Opaque — bind it via
/// [`BindGroupBuilder::texture_cubemap`](super::buffers::BindGroupBuilder::texture_cubemap).
///
/// [`empty`](CubemapBuilder::empty)'s documented use case — rendering
/// into per-face views for environment capture (a skybox capture, an
/// irradiance/specular IBL prefilter pass, a reflection probe, ...) — is
/// [`face_attachment`](Self::face_attachment).
pub struct GPUCubemap {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: u32,
    format: TextureFormat,
    ctx: GpuContext,
}

impl GPUCubemap {
    /// Overwrites one face's level-0 pixel data (+X, -X, +Y, -Y, +Z, -Z is
    /// `face` 0..=5, matching [`CubemapBuilder::from_faces`]'s order).
    /// See [`GPUTexture::write`](super::textures::GPUTexture::write) for the
    /// same caveat about mip levels not being regenerated.
    pub fn write_face(&self, face: u32, pixels: &[u8]) {
        write_texture_level0(self.ctx.queue(), &self.texture, face, self.format.into(), self.size, self.size, pixels);
    }

    /// A render-target view onto one face at one mip level, for rendering
    /// into directly — an environment-map capture, a specular IBL prefilter
    /// pass writing successive mip levels, a reflection probe. `face` is
    /// `0..=5` in the same order as [`CubemapBuilder::from_faces`]'s
    /// array (+X, -X, +Y, -Y, +Z, -Z); `mip_level` is `0` unless this
    /// cubemap was built [`with_mips`](CubemapBuilder::with_mips), in
    /// which case a prefilter pass typically writes one mip level per
    /// roughness step.
    ///
    /// Only meaningful for a cubemap allocated with `RENDER_ATTACHMENT`
    /// usage, which [`CubemapBuilder::empty`] sets automatically.
    /// Panics if `face` is out of range.
    pub fn face_attachment(&self, face: u32, mip_level: u32) -> super::texture_view::TextureView {
        assert!(face < 6, "GPUCubemap::face_attachment: face {face} out of range (0..=5)");
        let view = self.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: mip_level,
            mip_level_count: Some(1),
            base_array_layer: face,
            array_layer_count: Some(1),
            ..Default::default()
        });
        super::texture_view::TextureView::from_raw(view, self.texture.clone())
    }

    /// Edge length in pixels.
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

impl Asset<WGPUBackend> for Cubemap {
    type Deps<'a> = Res<'a, MipmapGenerator>;

    fn upload<'a>(
        &self,
        backend: &WGPUBackend,
        mipmap_generator: &Res<'a, MipmapGenerator>,
    ) -> Option<GPUCubemap> {
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

        let mip_count = super::mipmap::mip_count(self.size, self.generate_mips);

        let texture = backend
            .device
            .create_texture(&self.wgpu_descriptor(mip_count, faces.is_none()));

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
                        bytes_per_row: Some(bytes_per_pixel(self.format.into()) * self.size),
                        rows_per_image: Some(self.size),
                    },
                    wgpu::Extent3d {
                        width: self.size,
                        height: self.size,
                        depth_or_array_layers: 1,
                    },
                );
            }

            if mip_count > 1 {
                mipmap_generator.generate_mips(
                    &backend.device,
                    &backend.queue,
                    &texture,
                    self.format.into(),
                    mip_count,
                    6,
                );
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        Some(GPUCubemap {
            texture,
            view,
            size: self.size,
            format: self.format,
            ctx: GpuContext::from_backend(backend),
        })
    }
}

crate::wgpu::plugin_macros::mipmap_asset_plugin! {
    /// Registers the [`GPUCubemap`] asset pipeline (`Assets<Cubemap>`),
    /// plus the [`MipmapGenerator`] it depends on for `generate_mips`. Included
    /// by [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    CubemapPlugin, Cubemap
}
