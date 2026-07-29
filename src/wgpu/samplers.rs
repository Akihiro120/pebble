#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerKind {
    LinearRepeat,
    LinearClamp,
    /// Like `LinearClamp`, but clamped to mip 0 only (`lod_min_clamp`/
    /// `lod_max_clamp` both 0.0). Use this when you need to force sampling
    /// the base level regardless of how many mips the texture actually
    /// has — e.g. an environment-map capture pass where blending across
    /// mips would introduce blur you don't want in the baked result.
    LinearClampNoMip,
    Nearest,
    /// Nearest-filtered (min/mag), clamped to an opaque white border rather
    /// than the edge texel — matches the classic GL texture-array setup:
    /// `GL_NEAREST` + `GL_CLAMP_TO_BORDER` + a `{1,1,1,1}` border color.
    /// Useful for a `texture_2d_array` where sampling past a layer's edge
    /// should read as "nothing here" (white) instead of edge bleed.
    NearestClampBorder,
}

impl SamplerKind {
    /// Pure conversion to a wgpu descriptor. You still call
    /// `device.create_sampler(&kind.descriptor())` yourself, in your own
    /// `LazyResource::construct` — this only fixes the handful of
    /// address-mode/filter combinations so you don't re-derive them.
    pub fn descriptor(&self) -> wgpu::SamplerDescriptor<'static> {
        match self {
            SamplerKind::LinearRepeat => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::LinearClamp => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::LinearClampNoMip => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_min_clamp: 0.0,
                lod_max_clamp: 0.0,
                ..Default::default()
            },
            SamplerKind::Nearest => wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::NearestClampBorder => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToBorder,
                address_mode_v: wgpu::AddressMode::ClampToBorder,
                address_mode_w: wgpu::AddressMode::ClampToBorder,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                border_color: Some(wgpu::SamplerBorderColor::OpaqueWhite),
                ..Default::default()
            },
        }
    }
}
