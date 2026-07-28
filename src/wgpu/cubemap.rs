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
    pub fn wgpu_descriptor(&self) -> wgpu::TextureDescriptor<'static> {
        let usage = match self.faces {
            Some(_) => wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            None => {
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
            }
        };

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
            usage,
            view_formats: &[],
        }
    }

    /// The one part that's easy to get wrong by hand: a cubemap's VIEW
    /// must be created with `TextureViewDimension::Cube`, not the
    /// default. This is the view you sample from; use
    /// [`face_view_descriptor`](Self::face_view_descriptor) to get a
    /// single-layer `D2` view for rendering into one face.
    pub fn view_descriptor(&self) -> wgpu::TextureViewDescriptor<'static> {
        wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            ..Default::default()
        }
    }

    /// A single-face `D2` view over layer `face` (0..6, wgpu's +X, -X, +Y,
    /// -Y, +Z, -Z order) — what you attach as a `RenderPassColorAttachment`
    /// to draw into that one face of an [`empty`](Self::empty) cubemap.
    pub fn face_view_descriptor(&self, face: u32) -> wgpu::TextureViewDescriptor<'static> {
        wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: face,
            array_layer_count: Some(1),
            ..Default::default()
        }
    }
}

impl CubemapSpec {
    pub fn new(size: u32, faces: [Vec<u8>; 6]) -> Self {
        Self {
            size,
            format: wgpu::TextureFormat::Rgba8Unorm,
            faces: Some(faces),
        }
    }

    /// An empty cubemap with no data uploaded — allocated with
    /// `RENDER_ATTACHMENT` usage so it can be filled later by rendering
    /// into each face (via [`face_view_descriptor`](Self::face_view_descriptor)),
    /// e.g. an environment-map capture pass.
    pub fn empty(size: u32) -> Self {
        Self {
            size,
            format: wgpu::TextureFormat::Rgba8Unorm,
            faces: None,
        }
    }

    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}
