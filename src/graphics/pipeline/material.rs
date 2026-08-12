use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupLayout, BindGroupLayoutBuilder, BindGroupTarget, BindingEntry},
            layout::{assemble_group_layouts, find_own_entries, GlobalLayoutPool, GroupEntry, PipelineKind},
        },
        render::Backend,
        types::{
            Face, PolygonMode,
            flags::ShaderStages,
            pipeline_state::{ColorTargetState, DepthStencilState, VertexBufferLayout},
        },
    },
};

pub struct RenderPipeline(wgpu::RenderPipeline);

impl RenderPipeline {
    pub(crate) fn raw(&self) -> &wgpu::RenderPipeline {
        &self.0
    }
}

pub struct Material {
    label: Option<&'static str>,
    shader_source: &'static str,
    vertex_entry: Option<&'static str>,
    fragment_entry: Option<&'static str>,
    vertex_layouts: Vec<VertexBufferLayout>,
    groups: Vec<GroupEntry>,
    cull_mode: Option<Face>,
    depth: Option<DepthStencilState>,
    targets: Vec<ColorTargetState>,
    polygon_mode: PolygonMode,
    sample_count: u32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: Vec::new(),
            groups: Vec::new(),
            cull_mode: Some(Face::Back),
            depth: None,
            targets: Vec::new(),
            polygon_mode: PolygonMode::Fill,
            sample_count: 1,
        }
    }
}

impl Material {
    pub fn new(shader_source: &'static str) -> Self {
        Self { shader_source, ..Self::default() }
    }

    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_vertex_entry(mut self, entry: &'static str) -> Self {
        self.vertex_entry = Some(entry);
        self
    }

    pub fn without_vertex_entry(mut self) -> Self {
        self.vertex_entry = None;
        self
    }

    pub fn with_fragment_entry(mut self, entry: &'static str) -> Self {
        self.fragment_entry = Some(entry);
        self
    }

    pub fn without_fragment_entry(mut self) -> Self {
        self.fragment_entry = None;
        self
    }

    pub fn with_vertex_layouts(mut self, layouts: Vec<VertexBufferLayout>) -> Self {
        self.vertex_layouts = layouts;
        self
    }

    pub fn with_entries(mut self, groups: Vec<GroupEntry>) -> Self {
        self.groups = groups;
        self
    }

    pub fn with_cull_mode(mut self, mode: Face) -> Self {
        self.cull_mode = Some(mode);
        self
    }

    pub fn without_cull_mode(mut self) -> Self {
        self.cull_mode = None;
        self
    }

    pub fn with_depth(mut self, depth: DepthStencilState) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn with_targets(mut self, targets: Vec<ColorTargetState>) -> Self {
        self.targets = targets;
        self
    }

    pub fn with_polygon_mode(mut self, mode: PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    fn validate(&self) {
        if self.targets.is_empty() {
            tracing::warn!(
                "Material{}: no color targets set — a render pipeline normally writes to \
                 at least one; consider calling .with_targets(...) (unless this is intentionally a \
                 depth-only pass)",
                self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Material>) -> Handle<Material> {
        self.validate();
        assets.insert(name, self)
    }
}

fn check_material_limits(device: &wgpu::Device, desc: &Material) {
    let limits = device.limits();
    let labeled = || desc.label.map(|l| format!(" '{l}'")).unwrap_or_default();

    let buffer_count = desc.vertex_layouts.len() as u32;
    if buffer_count > limits.max_vertex_buffers {
        panic!(
            "material{}: {buffer_count} vertex buffer layouts exceeds this device's \
             max_vertex_buffers ({})",
            labeled(),
            limits.max_vertex_buffers
        );
    }

    let attribute_count: u32 = desc.vertex_layouts.iter().map(|l| l.attributes.len() as u32).sum();
    if attribute_count > limits.max_vertex_attributes {
        panic!(
            "material{}: {attribute_count} vertex attributes (summed across every vertex \
             layout) exceeds this device's max_vertex_attributes ({})",
            labeled(),
            limits.max_vertex_attributes
        );
    }

    let target_count = desc.targets.len() as u32;
    if target_count > limits.max_color_attachments {
        panic!(
            "material{}: {target_count} color targets exceeds this device's max_color_attachments ({})",
            labeled(),
            limits.max_color_attachments
        );
    }
}

pub fn build_material(backend: &Backend, desc: &Material, pool: &GlobalLayoutPool) -> Option<(RenderPipeline, BindGroupLayout)> {
    check_material_limits(&backend.device, desc);

    let own_entries = find_own_entries(desc.label, PipelineKind::Material, &desc.groups);
    for entry in own_entries {
        if entry.kind.visibility().intersects(ShaderStages::COMPUTE) {
            panic!(
                "material{}: entry '{}' is visible to the compute stage — material bind \
                 group entries must not be COMPUTE-visible",
                desc.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                entry.name,
            );
        }
    }

    let layout = BindGroupLayoutBuilder::new()
        .with_label(desc.label)
        .with_entries(own_entries.iter().cloned())
        .build(backend);

    let device = &backend.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: desc.label,
        source: wgpu::ShaderSource::Wgsl(desc.shader_source.into()),
    });

    let bind_group_layouts = assemble_group_layouts(
        desc.label,
        &desc.groups,
        &layout,
        pool,
        device.limits().max_bind_groups,
    )?;

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let attribute_sets: Vec<Vec<wgpu::VertexAttribute>> = desc
        .vertex_layouts
        .iter()
        .map(|l| l.attributes.iter().map(|a| (*a).into()).collect())
        .collect();
    let vertex_buffers: Vec<wgpu::VertexBufferLayout> = desc
        .vertex_layouts
        .iter()
        .zip(attribute_sets.iter())
        .map(|(l, attrs)| wgpu::VertexBufferLayout {
            array_stride: l.array_stride,
            step_mode: l.step_mode.into(),
            attributes: attrs,
        })
        .collect();

    let targets: Vec<Option<wgpu::ColorTargetState>> =
        desc.targets.iter().cloned().map(|t| Some(t.into())).collect();

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: desc.vertex_entry,
            compilation_options: Default::default(),
            buffers: &vertex_buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: desc.cull_mode.map(Into::into),
            unclipped_depth: false,
            polygon_mode: desc.polygon_mode.into(),
            conservative: false,
        },
        depth_stencil: desc.depth.clone().map(Into::into),
        multisample: wgpu::MultisampleState {
            count: desc.sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: desc.fragment_entry,
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    });

    Some((RenderPipeline(pipeline), layout))
}

pub struct GPUMaterial {
    pub pipeline: RenderPipeline,
    layout: BindGroupLayout,
    entries: Vec<BindingEntry>,
}

impl BindGroupTarget for GPUMaterial {
    fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.layout
    }
    fn binding_entries(&self) -> &[BindingEntry] {
        &self.entries
    }
}

impl AssetSource for Material {
    type Processed = GPUMaterial;
}

impl Asset<Backend> for Material {
    type Deps<'a> = Read<'a, GlobalLayoutPool>;

    fn upload<'a>(&self, backend: &Backend, pool: &Read<'a, GlobalLayoutPool>) -> Option<GPUMaterial> {
        let (pipeline, layout) = build_material(backend, self, pool)?;
        let entries = find_own_entries(self.label, PipelineKind::Material, &self.groups).to_vec();

        Some(GPUMaterial { pipeline, layout, entries })
    }
}
