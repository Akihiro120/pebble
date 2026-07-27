pub struct TextureSpec {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub data: Vec<u8>,
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
