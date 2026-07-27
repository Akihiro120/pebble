pub mod cubemap;
pub mod material;
pub mod material_instance;
pub mod samplers;
pub mod textures;

pub struct BindingEntry {
    pub name: &'static str,
    pub kind: BindingKind,
}

pub enum BindingKind {
    Texture,
    Sampler,
    Buffer,
    TextureCubemap,
}

impl BindingEntry {
    pub fn texture(name: &'static str) -> Self {
        Self {
            name,
            kind: BindingKind::Texture,
        }
    }

    pub fn sampler(name: &'static str) -> Self {
        Self {
            name,
            kind: BindingKind::Sampler,
        }
    }

    pub fn buffer(name: &'static str) -> Self {
        Self {
            name,
            kind: BindingKind::Buffer,
        }
    }

    pub fn cubemap(name: &'static str) -> Self {
        Self {
            name,
            kind: BindingKind::TextureCubemap,
        }
    }

    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self.kind {
            BindingKind::Texture => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindingKind::Sampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            BindingKind::Buffer => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindingKind::TextureCubemap => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
        }
    }
}
