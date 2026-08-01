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
    /// The `@binding(N)` this entry occupies within its bind group. Explicit rather than
    /// inferred from position in `entries`, so it matches the shader unambiguously.
    pub binding: u32,
    pub kind: ComputeBindingKind,
}

pub struct ComputeDescriptor<'a> {
    pub label: Option<&'a str>,
    pub shader_source: &'a str,
    pub entry_point: Option<&'a str>,
    pub entries: Vec<ComputeBindingEntry>,
    /// Which `@group(N)` the layout built from `entries` occupies in the pipeline, or
    /// `None` if this compute pass has no entries of its own (e.g. it only uses `extra_layouts`).
    pub own_group: Option<u32>,
    /// Additional bind group layouts, each tagged with the `@group(N)` it occupies.
    /// Every index from 0 up to the highest one used (including `own_group`, if set) must
    /// be covered exactly once, or `build_compute` panics — this makes group assignment
    /// explicit instead of inferred from field order.
    pub extra_layouts: Vec<super::layout::OwnedGroupLayout>,
}

impl<'a> Default for ComputeDescriptor<'a> {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            entry_point: Some("cs_main"),
            entries: Vec::new(),
            own_group: Some(0),
            extra_layouts: Vec::new(),
        }
    }
}

pub fn build_bind_group_layout(
    device: &wgpu::Device,
    label: Option<&str>,
    entries: &[ComputeBindingEntry],
) -> wgpu::BindGroupLayout {
    let layout_entries: Vec<_> = entries.iter().map(|e| e.kind.layout_entry(e.binding)).collect();

    let mut seen = std::collections::HashSet::new();
    for e in entries {
        if !seen.insert(e.binding) {
            panic!(
                "binding {} assigned more than once building bind group layout{} (entry '{}')",
                e.binding,
                label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                e.name
            );
        }
    }

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

    let mut slots: Vec<super::layout::GroupLayout> = desc
        .extra_layouts
        .iter()
        .map(|g| super::layout::GroupLayout { group: g.group, layout: &g.layout })
        .collect();
    if let Some(own_group) = desc.own_group {
        slots.push(super::layout::GroupLayout { group: own_group, layout: &layout });
    }
    let bind_group_layouts = super::layout::assemble_bind_group_layouts(desc.label, slots);

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
