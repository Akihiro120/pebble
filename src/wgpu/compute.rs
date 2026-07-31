use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ComputeBindingKind {
    StorageBufferReadOnly,
    StorageBufferReadWrite,
    UniformBuffer,
    Texture,
    StorageTexture {
        format: wgpu::TextureFormat,
        access: wgpu::StorageTextureAccess,
    },
    Sampler,
}

impl ComputeBindingKind {
    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            ComputeBindingKind::StorageBufferReadOnly => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            ComputeBindingKind::StorageBufferReadWrite => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            ComputeBindingKind::UniformBuffer => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            ComputeBindingKind::Texture => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            ComputeBindingKind::StorageTexture { format, access } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: *access,
                    format: *format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            ComputeBindingKind::Sampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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

pub fn build_compute(
    device: &wgpu::Device,
    desc: &ComputeDescriptor,
) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let layout_entries: Vec<_> = desc
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| e.kind.layout_entry(i as u32))
        .collect();

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: desc.label,
        entries: &layout_entries,
    });

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
