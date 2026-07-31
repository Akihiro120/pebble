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
    pub vertex_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    pub entries: Vec<MaterialBindingEntry>,
    pub cull_mode: Option<wgpu::Face>,
    pub depth: Option<wgpu::DepthStencilState>,
    pub targets: Vec<wgpu::ColorTargetState>,
    pub polygon_mode: wgpu::PolygonMode,
    pub extra_layouts: Vec<wgpu::BindGroupLayout>,
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

pub fn build_uniform_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    contents: &[u8],
) -> (wgpu::Buffer, wgpu::BindGroup) {
    use wgpu::util::DeviceExt;

    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    (buffer, bind_group)
}

pub fn update_uniform_buffer(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &[u8]) {
    queue.write_buffer(buffer, 0, data);
}

pub fn build_material(
    device: &wgpu::Device,
    desc: &MaterialDescriptor,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let layout = build_bind_group_layout(&device, desc.label, &desc.entries);

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
