pub struct TextureSpec {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// Base (mip 0) pixel data, tightly packed, 4 bytes per texel (matching
    /// `format`). Required even when `format` isn't literally `Rgba8*` —
    /// mip generation always treats it as 4 bytes/texel.
    pub data: Vec<u8>,
    pub generate_mips: bool,
}

impl TextureSpec {
    /// Number of mip levels this texture will have: the full chain down to
    /// 1x1 if `generate_mips`, otherwise just the base level.
    pub fn mip_level_count(&self) -> u32 {
        if self.generate_mips {
            32 - self.width.max(self.height).max(1).leading_zeros()
        } else {
            1
        }
    }

    /// Pure conversion — matches what [`upload`](Self::upload) actually
    /// creates, including `mip_level_count`. You can call
    /// `device.create_texture(&spec.wgpu_descriptor())` yourself if you'd
    /// rather write the mip levels some other way; [`upload`](Self::upload)
    /// is the batteries-included path.
    pub fn wgpu_descriptor(&self) -> wgpu::TextureDescriptor<'static> {
        wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: self.mip_level_count(),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }
    }

    /// Box-filter downsample to half size (minimum 1x1), 4 bytes/texel.
    fn downsample(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
        let next_width = (width / 2).max(1);
        let next_height = (height / 2).max(1);
        let mut out = vec![0u8; (next_width * next_height * 4) as usize];

        for y in 0..next_height {
            for x in 0..next_width {
                let x0 = (x * 2).min(width - 1);
                let x1 = (x * 2 + 1).min(width - 1);
                let y0 = (y * 2).min(height - 1);
                let y1 = (y * 2 + 1).min(height - 1);

                for c in 0..4 {
                    let sample = |sx: u32, sy: u32| -> u32 {
                        data[((sy * width + sx) * 4 + c) as usize] as u32
                    };
                    let avg = (sample(x0, y0) + sample(x1, y0) + sample(x0, y1) + sample(x1, y1))
                        / 4;
                    out[((y * next_width + x) * 4 + c) as usize] = avg as u8;
                }
            }
        }

        (out, next_width, next_height)
    }

    /// Create the texture, write every mip level (generating each one from
    /// the last via a box filter if `generate_mips` is set), and return it
    /// along with a default view over the whole chain.
    pub fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&self.wgpu_descriptor());
        let mip_count = self.mip_level_count();

        let mut level_data = self.data.clone();
        let mut level_width = self.width;
        let mut level_height = self.height;

        for level in 0..mip_count {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level,
                    origin: wgpu::Origin3d::default(),
                    aspect: wgpu::TextureAspect::All,
                },
                &level_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * level_width),
                    rows_per_image: Some(level_height),
                },
                wgpu::Extent3d {
                    width: level_width,
                    height: level_height,
                    depth_or_array_layers: 1,
                },
            );

            if level + 1 < mip_count {
                let (next_data, next_width, next_height) =
                    Self::downsample(&level_data, level_width, level_height);
                level_data = next_data;
                level_width = next_width;
                level_height = next_height;
            }
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
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
