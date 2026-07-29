pub struct CubemapSpec {
    pub size: u32, // cubemaps are square per face
    pub format: wgpu::TextureFormat,
    /// `Some` uploads 6 faces of pixel data up front (wgpu's expected
    /// order: +X, -X, +Y, -Y, +Z, -Z). `None` allocates an empty cubemap
    /// meant to be filled later by rendering into per-face views — e.g. an
    /// environment-map capture pass — in which case [`wgpu_descriptor`](Self::wgpu_descriptor)
    /// adds `RENDER_ATTACHMENT` usage instead of requiring upload data.
    pub faces: Option<[Vec<u8>; 6]>,
}

impl CubemapSpec {
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}
