pub struct CubemapSpec {
    pub size: u32, // cubemaps are square per face
    pub format: wgpu::TextureFormat,
    /// Exactly 6 faces, in wgpu's expected order: +X, -X, +Y, -Y, +Z, -Z.
    pub faces: [Vec<u8>; 6],
}

impl CubemapSpec {
    pub fn wgpu_descriptor(&self) -> wgpu::TextureDescriptor<'static> {
        wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }
    }

    /// The one part that's easy to get wrong by hand: a cubemap's VIEW
    /// must be created with `TextureViewDimension::Cube`, not the
    /// default. You still call `texture.create_view(&spec.view_descriptor())` yourself.
    pub fn view_descriptor(&self) -> wgpu::TextureViewDescriptor<'static> {
        wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        }
    }
}

impl CubemapSpec {
    pub fn new(size: u32, faces: [Vec<u8>; 6]) -> Self {
        Self {
            size,
            format: wgpu::TextureFormat::Rgba8Unorm,
            faces,
        }
    }

    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}
