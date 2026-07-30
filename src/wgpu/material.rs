use crate::{
    app::App,
    assets::{plugin::AssetPlugin, upload::Asset},
    ecs::plugin::Plugin,
    wgpu::backend::WGPUBackend,
};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum MaterialBindingKind {
    Texture,
    TextureArray,
    Sampler,
    ComparisonSampler,
    Buffer,
    TextureCubemap,
}

impl MaterialBindingKind {
    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            MaterialBindingKind::Texture => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            MaterialBindingKind::TextureArray => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
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
            MaterialBindingKind::Buffer => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            MaterialBindingKind::TextureCubemap => wgpu::BindGroupLayoutEntry {
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

#[derive(Clone)]
pub struct MaterialBindingEntry {
    pub name: &'static str,
    pub kind: MaterialBindingKind,
}

pub struct MaterialDescriptor<'a> {
    pub label: Option<&'a str>,
    pub shader_source: &'a str,
    pub vertex_entry: Option<&'a str>,
    pub fragment_entry: Option<&'a str>,
    pub vertex_layouts: &'a [wgpu::VertexBufferLayout<'a>],
    pub entries: &'a [MaterialBindingEntry],
    pub cull_mode: Option<wgpu::Face>,
    pub depth: Option<wgpu::DepthStencilState>,
    pub targets: &'a [wgpu::ColorTargetState],
    pub polygon_mode: wgpu::PolygonMode,
    pub extra_layouts: &'a [&'a wgpu::BindGroupLayout],
}

const DEFAULT_TARGET: [wgpu::ColorTargetState; 1] = [wgpu::ColorTargetState {
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
            vertex_layouts: &[],
            entries: &[],
            cull_mode: Some(wgpu::Face::Back),
            depth: None,
            targets: &[],
            extra_layouts: &[],
            polygon_mode: wgpu::PolygonMode::Fill,
        }
    }
}

pub fn build_material(
    device: &wgpu::Device,
    desc: &MaterialDescriptor,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
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

    let mut bind_group_layouts: Vec<_> = desc.extra_layouts.iter().map(|l| Some(*l)).collect();
    bind_group_layouts.push(Some(&layout));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let targets: Vec<_> = desc.targets.iter().cloned().map(Some).collect();

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: desc.vertex_entry,
            compilation_options: Default::default(),
            buffers: desc.vertex_layouts,
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
        let (pipeline, layout) = build_material(&backend.device, &source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

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
