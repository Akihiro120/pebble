use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum MaterialBindingKind {
    Texture {
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
    },
    Sampler,
    ComparisonSampler,
    UniformBuffer {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    StorageBufferReadOnly {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    StorageBufferReadWrite {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
}

impl MaterialBindingKind {
    pub fn texture_2d() -> Self {
        Self::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    }

    pub fn texture_2d_array() -> Self {
        Self::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        }
    }

    pub fn texture_cubemap() -> Self {
        Self::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::Cube,
            multisampled: false,
        }
    }

    pub fn uniform_buffer() -> Self {
        Self::UniformBuffer {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            has_dynamic_offset: false,
            min_binding_size: None,
        }
    }

    /// A uniform buffer bound with a per-draw dynamic offset, e.g. one large buffer
    /// holding many objects' data, rebound at a different offset via
    /// `RenderPass::set_bind_group`'s dynamic offsets slice instead of a bind group per object.
    /// `element_size` is the size in bytes of a single element (before alignment padding).
    /// Use [`crate::wgpu::buffers::build_dynamic_uniform_buffer`] to allocate the backing
    /// buffer and [`crate::wgpu::buffers::dynamic_buffer_binding`] (not
    /// `buffer.as_entire_binding()`) to build the bind group entry for it — the entry must
    /// be scoped to one element's size, not the whole buffer, or dynamic offsets will fail
    /// validation.
    pub fn dynamic_uniform_buffer(element_size: u64) -> Self {
        Self::UniformBuffer {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(element_size),
        }
    }

    /// A storage buffer bound with a per-draw dynamic offset. See [`Self::dynamic_uniform_buffer`].
    pub fn dynamic_storage_buffer(element_size: u64, read_only: bool) -> Self {
        let visibility = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let has_dynamic_offset = true;
        let min_binding_size = wgpu::BufferSize::new(element_size);
        if read_only {
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size }
        } else {
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size }
        }
    }

    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            MaterialBindingKind::Texture { sample_type, view_dimension, multisampled } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: *sample_type,
                    view_dimension: *view_dimension,
                    multisampled: *multisampled,
                },
                count: None,
            },
            MaterialBindingKind::Sampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            MaterialBindingKind::ComparisonSampler => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            MaterialBindingKind::UniformBuffer { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            MaterialBindingKind::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            MaterialBindingKind::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
        }
    }
}

#[derive(Clone)]
pub struct MaterialBindingEntry {
    pub name: &'static str,
    /// The `@binding(N)` this entry occupies within its bind group. Explicit rather than
    /// inferred from position in `entries`, so it matches the shader unambiguously.
    pub binding: u32,
    pub kind: MaterialBindingKind,
}

pub struct MaterialDescriptor<'a> {
    pub label: Option<&'a str>,
    pub shader_source: &'a str,
    pub vertex_entry: Option<&'a str>,
    pub fragment_entry: Option<&'a str>,
    pub vertex_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    pub entries: Vec<MaterialBindingEntry>,
    pub cull_mode: Option<wgpu::Face>,
    pub depth: Option<wgpu::DepthStencilState>,
    pub targets: Vec<wgpu::ColorTargetState>,
    pub polygon_mode: wgpu::PolygonMode,
    /// Which `@group(N)` the layout built from `entries` occupies in the pipeline.
    pub own_group: u32,
    /// Additional bind group layouts, each tagged with the `@group(N)` it occupies.
    /// Every index from 0 up to the highest one used (including `own_group`) must be
    /// covered exactly once, or `build_material` panics — this makes group assignment
    /// explicit instead of inferred from field order.
    pub extra_layouts: Vec<super::layout::OwnedGroupLayout>,
}

pub const DEFAULT_TARGET: [wgpu::ColorTargetState; 1] = [wgpu::ColorTargetState {
    format: wgpu::TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: wgpu::ColorWrites::ALL,
}];

impl<'a> Default for MaterialDescriptor<'a> {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: Vec::new(),
            entries: Vec::new(),
            cull_mode: Some(wgpu::Face::Back),
            depth: None,
            targets: Vec::new(),
            own_group: 0,
            extra_layouts: Vec::new(),
            polygon_mode: wgpu::PolygonMode::Fill,
        }
    }
}

pub fn build_bind_group_layout(
    device: &wgpu::Device,
    label: Option<&str>,
    entries: &[MaterialBindingEntry],
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

pub fn build_material(
    device: &wgpu::Device,
    desc: &MaterialDescriptor,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
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
    slots.push(super::layout::GroupLayout { group: desc.own_group, layout: &layout });
    let bind_group_layouts = super::layout::assemble_bind_group_layouts(desc.label, slots);

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let targets: Vec<Option<wgpu::ColorTargetState>> =
        desc.targets.iter().cloned().map(Some).collect();

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: desc.vertex_entry,
            compilation_options: Default::default(),
            buffers: &desc.vertex_layouts,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: desc.cull_mode,
            unclipped_depth: false,
            polygon_mode: desc.polygon_mode,
            conservative: false,
        },
        depth_stencil: desc.depth.clone(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: desc.fragment_entry,
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, layout)
}

pub struct GPUMaterial {
    pub pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
    pub entries: Vec<MaterialBindingEntry>,
}

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = MaterialDescriptor<'static>;
    type Deps<'a> = ();

    fn upload<'a>(source: &MaterialDescriptor, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let (pipeline, layout) = build_material(&backend.device, source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

#[derive(Default)]
pub struct MaterialPlugin;
impl MaterialPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for MaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AssetPlugin::<super::backend::WGPUBackend, GPUMaterial>::new());
    }
}
