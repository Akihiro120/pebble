pub struct TextureSpec {
    pub file: Option<&'static str>,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub data: Option<Vec<u8>>,
    pub generate_mips: bool,
}

impl TextureSpec {
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }
}
