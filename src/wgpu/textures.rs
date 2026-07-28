pub struct TextureSpec {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub data: Vec<u8>,
    pub generate_mips: bool,
}

impl TextureSpec {
    /// Pure conversion — you call `device.create_texture(&spec.wgpu_descriptor())` yourself.
    pub fn wgpu_descriptor(&self) -> wgpu::TextureDescriptor<'static> {
        wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }
    }
}

impl TextureSpec {
    pub fn new(width: u32, height: u32, data: Vec<u8>, generate_mips: bool) -> Self {
        Self {
            width,
            height,
            format: wgpu::TextureFormat::Rgba8Unorm,
            data,
            generate_mips,
        }
    }

    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}
