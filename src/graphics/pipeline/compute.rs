use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupLayout, BindGroupLayoutBuilder, BindGroupTarget, BindingEntry},
            layout::{assemble_group_layouts, find_own_entries, GlobalLayoutPool, GroupEntry, PipelineKind},
        },
        render::Backend,
        types::flags::ShaderStages,
    },
};

/// A compiled GPU compute pipeline, wrapping `wgpu::ComputePipeline`.
pub struct ComputePipeline(wgpu::ComputePipeline);

impl ComputePipeline {
    pub(crate) fn raw(&self) -> &wgpu::ComputePipeline {
        &self.0
    }
}

/// A compute pipeline asset — WGSL shader source plus its bind group
/// layout. Dispatch it via [`ComputeInstance`](super::instance::ComputeInstance)
/// and [`Backend::dispatch_compute`](crate::graphics::render::Backend::dispatch_compute).
pub struct Compute {
    label: Option<&'static str>,
    shader_source: &'static str,
    entry_point: Option<&'static str>,
    groups: Vec<GroupEntry>,
}

impl Default for Compute {
    fn default() -> Self {
        Self { label: None, shader_source: "", entry_point: Some("cs_main"), groups: Vec::new() }
    }
}

impl Compute {
    pub fn new(shader_source: &'static str) -> Self {
        Self { shader_source, ..Self::default() }
    }

    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_entry_point(mut self, entry: &'static str) -> Self {
        self.entry_point = Some(entry);
        self
    }

    pub fn with_entries(mut self, groups: Vec<GroupEntry>) -> Self {
        self.groups = groups;
        self
    }

    fn validate(&self) {
        if self.groups.is_empty() {
            tracing::warn!(
                "Compute{}: no bind groups at all — this pass can't read or write \
                 anything; consider calling .with_entries(...)",
                self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<Compute>) -> Handle<Compute> {
        self.validate();
        assets.insert(name, self)
    }
}

/// Compiles a [`Compute`] into a raw pipeline + bind group layout. Used
/// internally by the asset upload path; exposed for callers assembling
/// pipelines outside the usual [`Assets`] flow.
pub fn build_compute(backend: &Backend, desc: &Compute, pool: &GlobalLayoutPool) -> Option<(ComputePipeline, BindGroupLayout)> {
    let own_entries = find_own_entries(desc.label, PipelineKind::Compute, &desc.groups);
    for entry in own_entries {
        if entry.kind.visibility() != ShaderStages::COMPUTE {
            panic!(
                "compute pass{}: entry '{}' is not visible to exactly the compute stage — \
                 compute bind group entries must be visible to exactly COMPUTE",
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

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: desc.entry_point,
        compilation_options: Default::default(),
        cache: None,
    });

    Some((ComputePipeline(pipeline), layout))
}

/// The GPU-resident pipeline an uploaded [`Compute`] produces.
pub struct GPUCompute {
    pub pipeline: ComputePipeline,
    layout: BindGroupLayout,
    entries: Vec<BindingEntry>,
}

impl BindGroupTarget for GPUCompute {
    fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.layout
    }
    fn binding_entries(&self) -> &[BindingEntry] {
        &self.entries
    }
}

impl AssetSource for Compute {
    type Processed = GPUCompute;
}

impl Asset<Backend> for Compute {
    type Deps<'a> = Read<'a, GlobalLayoutPool>;

    fn upload<'a>(&self, backend: &Backend, pool: &Read<'a, GlobalLayoutPool>) -> Option<GPUCompute> {
        let (pipeline, layout) = build_compute(backend, self, pool)?;
        let entries = find_own_entries(self.label, PipelineKind::Compute, &self.groups).to_vec();

        Some(GPUCompute { pipeline, layout, entries })
    }
}
