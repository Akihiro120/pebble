use crate::graphics::{
    render::Backend,
    types::{StorageTextureAccess, TextureFormat, TextureSampleType, TextureViewDimension, flags::ShaderStages},
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BindingKind {
    Texture {
        visibility: ShaderStages,
        sample_type: TextureSampleType,
        view_dimension: TextureViewDimension,
        multisampled: bool,
    },
    StorageTexture {
        visibility: ShaderStages,
        format: TextureFormat,
        access: StorageTextureAccess,
        view_dimension: TextureViewDimension,
    },
    Sampler { visibility: ShaderStages },
    ComparisonSampler { visibility: ShaderStages },
    UniformBuffer {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
    StorageBufferReadOnly {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
    StorageBufferReadWrite {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
}

impl BindingKind {
    pub fn texture_2d(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        }
    }

    pub fn texture_2d_array(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2Array,
            multisampled: false,
        }
    }

    pub fn texture_cubemap(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::Cube,
            multisampled: false,
        }
    }

    pub fn storage_texture(
        visibility: ShaderStages,
        format: TextureFormat,
        access: StorageTextureAccess,
        view_dimension: TextureViewDimension,
    ) -> Self {
        Self::StorageTexture { visibility, format, access, view_dimension }
    }

    pub fn sampler(visibility: ShaderStages) -> Self {
        Self::Sampler { visibility }
    }

    pub fn comparison_sampler(visibility: ShaderStages) -> Self {
        Self::ComparisonSampler { visibility }
    }

    pub fn uniform_buffer(visibility: ShaderStages) -> Self {
        Self::UniformBuffer { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    pub fn dynamic_uniform_buffer(visibility: ShaderStages, element_size: u64) -> Self {
        Self::UniformBuffer {
            visibility,
            has_dynamic_offset: true,
            min_binding_size: Some(element_size),
        }
    }

    pub fn storage_buffer_read_only(visibility: ShaderStages) -> Self {
        Self::StorageBufferReadOnly { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    pub fn storage_buffer_read_write(visibility: ShaderStages) -> Self {
        Self::StorageBufferReadWrite { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    pub fn dynamic_storage_buffer(visibility: ShaderStages, element_size: u64, read_only: bool) -> Self {
        let has_dynamic_offset = true;
        let min_binding_size = Some(element_size);
        if read_only {
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size }
        } else {
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size }
        }
    }

    pub fn visibility(&self) -> ShaderStages {
        match self {
            Self::Texture { visibility, .. }
            | Self::StorageTexture { visibility, .. }
            | Self::Sampler { visibility }
            | Self::ComparisonSampler { visibility }
            | Self::UniformBuffer { visibility, .. }
            | Self::StorageBufferReadOnly { visibility, .. }
            | Self::StorageBufferReadWrite { visibility, .. } => *visibility,
        }
    }

    pub(crate) fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            Self::Texture { visibility, sample_type, view_dimension, multisampled } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Texture {
                    sample_type: (*sample_type).into(),
                    view_dimension: (*view_dimension).into(),
                    multisampled: *multisampled,
                },
                count: None,
            },
            Self::StorageTexture { visibility, format, access, view_dimension } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::StorageTexture {
                    access: (*access).into(),
                    format: (*format).into(),
                    view_dimension: (*view_dimension).into(),
                },
                count: None,
            },
            Self::Sampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            Self::ComparisonSampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            Self::UniformBuffer { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
        }
    }
}

#[derive(Clone)]
pub struct BindGroupLayout(wgpu::BindGroupLayout);

impl BindGroupLayout {
    pub(crate) fn raw(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

#[derive(Clone)]
pub struct BindingEntry {
    pub name: &'static str,
    pub binding: u32,
    pub kind: BindingKind,
}

#[derive(Default)]
pub struct BindGroupLayoutBuilder<'a> {
    label: Option<&'a str>,
    entries: Vec<BindingEntry>,
}

impl<'a> BindGroupLayoutBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_entry(mut self, name: &'static str, binding: u32, kind: BindingKind) -> Self {
        self.entries.push(BindingEntry { name, binding, kind });
        self
    }

    pub fn with_entries(mut self, entries: impl IntoIterator<Item = BindingEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    pub fn build(self, backend: &Backend) -> BindGroupLayout {
        let layout_entries: Vec<_> =
            self.entries.iter().map(|e| e.kind.layout_entry(e.binding)).collect();

        let mut seen = std::collections::HashSet::new();
        for e in &self.entries {
            if !seen.insert(e.binding) {
                panic!(
                    "binding {} assigned more than once building bind group layout{} (entry '{}')",
                    e.binding,
                    self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                    e.name
                );
            }
        }

        BindGroupLayout(backend.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: self.label,
            entries: &layout_entries,
        }))
    }
}

pub trait BindGroupTarget {
    fn bind_group_layout(&self) -> &BindGroupLayout;
    fn binding_entries(&self) -> &[BindingEntry];
}
