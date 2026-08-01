use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ComputeBindingKind {
    StorageBufferReadOnly {
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    StorageBufferReadWrite {
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    UniformBuffer {
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    Texture {
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
    },
    StorageTexture {
        format: wgpu::TextureFormat,
        access: wgpu::StorageTextureAccess,
        view_dimension: wgpu::TextureViewDimension,
    },
    Sampler,
    ComparisonSampler,
}

impl ComputeBindingKind {
    pub fn texture_2d() -> Self {
        Self::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    }

    pub fn storage_buffer_read_only() -> Self {
        Self::StorageBufferReadOnly { has_dynamic_offset: false, min_binding_size: None }
    }

    pub fn storage_buffer_read_write() -> Self {
        Self::StorageBufferReadWrite { has_dynamic_offset: false, min_binding_size: None }
    }

    pub fn uniform_buffer() -> Self {
        Self::UniformBuffer { has_dynamic_offset: false, min_binding_size: None }
    }

    /// A uniform buffer bound with a per-dispatch dynamic offset, e.g. one large buffer
    /// holding many elements' data, rebound at a different offset via
    /// `ComputePass::set_bind_group`'s dynamic offsets slice instead of a bind group per
    /// dispatch. `element_size` is the size in bytes of a single element (before alignment
    /// padding). Use [`crate::wgpu::buffers::build_dynamic_uniform_buffer`] to allocate the
    /// backing buffer and [`crate::wgpu::buffers::dynamic_buffer_binding`] (not
    /// `buffer.as_entire_binding()`) to build the bind group entry for it — the entry must
    /// be scoped to one element's size, not the whole buffer, or dynamic offsets will fail
    /// validation.
    pub fn dynamic_uniform_buffer(element_size: u64) -> Self {
        Self::UniformBuffer { has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(element_size) }
    }

    /// A storage buffer bound with a per-dispatch dynamic offset. See [`Self::dynamic_uniform_buffer`].
    pub fn dynamic_storage_buffer(element_size: u64, read_only: bool) -> Self {
        let has_dynamic_offset = true;
        let min_binding_size = wgpu::BufferSize::new(element_size);
        if read_only {
            Self::StorageBufferReadOnly { has_dynamic_offset, min_binding_size }
        } else {
            Self::StorageBufferReadWrite { has_dynamic_offset, min_binding_size }
        }
    }

    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            ComputeBindingKind::StorageBufferReadOnly { has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            ComputeBindingKind::StorageBufferReadWrite { has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            ComputeBindingKind::UniformBuffer { has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            ComputeBindingKind::Texture { sample_type, view_dimension, multisampled } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: *sample_type,
                    view_dimension: *view_dimension,
                    multisampled: *multisampled,
                },
                count: None,
            },
            ComputeBindingKind::StorageTexture { format, access, view_dimension } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: *access,
                    format: *format,
                    view_dimension: *view_dimension,
                },
                count: None,
            },
            ComputeBindingKind::Sampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            ComputeBindingKind::ComparisonSampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        }
    }
}

#[derive(Clone)]
pub struct ComputeBindingEntry {
    pub name: &'static str,
    pub kind: ComputeBindingKind,
}

pub struct ComputeDescriptor<'a> {
    pub label: Option<&'a str>,
    pub shader_source: &'a str,
    pub entry_point: Option<&'a str>,
    pub entries: Vec<ComputeBindingEntry>,
    pub extra_layouts: Vec<wgpu::BindGroupLayout>,
}

impl<'a> Default for ComputeDescriptor<'a> {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            entry_point: Some("cs_main"),
            entries: Vec::new(),
            extra_layouts: Vec::new(),
        }
    }
}

pub fn build_bind_group_layout(
    device: &wgpu::Device,
    label: Option<&str>,
    entries: &[ComputeBindingEntry],
) -> wgpu::BindGroupLayout {
    let layout_entries: Vec<_> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| e.kind.layout_entry(i as u32))
        .collect();

    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label,
        entries: &layout_entries,
    })
}

pub fn build_compute(
    device: &wgpu::Device,
    desc: &ComputeDescriptor,
) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let layout = build_bind_group_layout(device, desc.label, &desc.entries);

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: desc.label,
        source: wgpu::ShaderSource::Wgsl(desc.shader_source.into()),
    });

    let mut bind_group_layouts: Vec<&wgpu::BindGroupLayout> = desc.extra_layouts.iter().collect();
    bind_group_layouts.push(&layout);
    let bind_group_layouts: Vec<Option<&wgpu::BindGroupLayout>> =
        bind_group_layouts.into_iter().map(Some).collect();

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: desc.entry_point,
        compilation_options: Default::default(),
        cache: None,
    });

    (pipeline, layout)
}

pub struct GPUCompute {
    pub pipeline: wgpu::ComputePipeline,
    pub layout: wgpu::BindGroupLayout,
    pub entries: Vec<ComputeBindingEntry>,
}

impl Asset<WGPUBackend> for GPUCompute {
    type Source = ComputeDescriptor<'static>;
    type Deps<'a> = ();

    fn upload<'a>(source: &ComputeDescriptor, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let (pipeline, layout) = build_compute(&backend.device, source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

#[derive(Default)]
pub struct ComputePlugin;
impl ComputePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for ComputePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AssetPlugin::<super::backend::WGPUBackend, GPUCompute>::new());
    }
}
