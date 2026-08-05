use crate::{
    assets::{handle::Handle, storage::Assets, upload::Asset},
    wgpu::{
        backend::WGPUBackend,
        binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingEntry},
        flags::ShaderStages,
    },
};

/// A `wgpu::ComputePipeline`, opaque — built only via [`build_compute`]/
/// [`GPUCompute`]'s `Asset::upload`. Bind it against a
/// [`ComputePass`](super::compute_pass::ComputePass) via
/// [`ComputePass::set_pipeline`](super::compute_pass::ComputePass::set_pipeline);
/// there's no way to reach the underlying `wgpu::ComputePipeline` from
/// outside this crate.
pub struct ComputePipeline(wgpu::ComputePipeline);

impl ComputePipeline {
    pub(crate) fn raw(&self) -> &wgpu::ComputePipeline {
        &self.0
    }
}

/// Describes a compute pipeline + its own bind group, the source type
/// [`GPUCompute`] is built from via [`build_compute`].
pub struct Compute {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    pub label: Option<&'static str>,
    /// WGSL source for the compute stage.
    pub shader_source: &'static str,
    /// Compute stage entry point. Defaults to `"cs_main"`.
    pub entry_point: Option<&'static str>,
    /// This compute pass's own bind group entries. See
    /// [`BindingKind`](super::binding::BindingKind) for what a
    /// compute-appropriate entry looks like — [`build_compute`] panics if
    /// any entry here isn't exactly `COMPUTE`-visible.
    pub entries: Vec<BindingEntry>,
    /// Which `@group(N)` the layout built from `entries` occupies in the pipeline, or
    /// `None` if this compute pass has no entries of its own (e.g. it only uses `extra_layouts`).
    pub own_group: Option<u32>,
    /// Additional bind group layouts, each tagged with the `@group(N)` it occupies.
    /// Every index from 0 up to the highest one used (including `own_group`, if set) must
    /// be covered exactly once, or `build_compute` panics — this makes group assignment
    /// explicit instead of inferred from field order.
    pub extra_layouts: Vec<super::layout::OwnedGroupLayout>,
}

impl Default for Compute {
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

impl Compute {
    /// Start building a compute pass with the given WGSL shader source.
    /// All other fields are set to their defaults (see [`Default`]).
    pub fn new(shader_source: &'static str) -> Self {
        Self { shader_source, ..Self::default() }
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn entry_point(mut self, entry: &'static str) -> Self {
        self.entry_point = Some(entry);
        self
    }

    pub fn entries(mut self, entries: Vec<BindingEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn own_group(mut self, group: u32) -> Self {
        self.own_group = Some(group);
        self
    }

    pub fn extra_layouts(mut self, layouts: Vec<super::layout::OwnedGroupLayout>) -> Self {
        self.extra_layouts = layouts;
        self
    }

    /// Consume the builder and return the finished [`Compute`] value.
    pub fn build(self) -> Self {
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Compute>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        assets.insert(name, self)
    }
}

/// Builds a compute pipeline and its own bind group layout from `desc`.
///
/// Panics if any of `desc.entries` isn't visible to exactly the compute
/// stage — [`BindingKind`](super::binding::BindingKind) is shared with
/// [`Material`](super::material::Material), and this is
/// the check that catches a material entry (`FRAGMENT`/`VERTEX_FRAGMENT`)
/// accidentally reused in a compute pass instead of letting it fail deep
/// inside wgpu with a less specific error. The bind group layout itself
/// comes from [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder).
/// The pipeline layout is assembled from `desc.own_group` (this pass's own
/// layout) plus `desc.extra_layouts`, keyed by explicit `@group(N)` —
/// panics on a gap or a collision across `0..=max`, turning a mismatched
/// `@group(N)` in the shader into an immediate, specific error instead of
/// an opaque wgpu validation failure at draw time.
pub fn build_compute(backend: &WGPUBackend, desc: &Compute) -> (ComputePipeline, BindGroupLayout) {
    build_compute_raw(&backend.device, desc)
}

/// Internal primitive behind [`build_compute`] — used directly only by
/// tests, which have a raw `wgpu::Device` but no full [`WGPUBackend`].
pub(crate) fn build_compute_raw(
    device: &wgpu::Device,
    desc: &Compute,
) -> (ComputePipeline, BindGroupLayout) {
    for entry in &desc.entries {
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
        .label(desc.label)
        .entries(desc.entries.iter().cloned())
        .build_raw(device);

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

    (ComputePipeline(pipeline), layout)
}

/// A compute pass uploaded to the GPU: a compute pipeline plus the bind
/// group layout entries it expects.
pub struct GPUCompute {
    pub pipeline: ComputePipeline,
    layout: BindGroupLayout,
    entries: Vec<BindingEntry>,
}

impl super::binding::BindGroupTarget for GPUCompute {
    fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.layout
    }
    fn binding_entries(&self) -> &[BindingEntry] {
        &self.entries
    }
}

impl Asset<WGPUBackend> for GPUCompute {
    type Source = Compute;
    type Deps<'a> = ();

    fn upload<'a>(source: &Compute, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let (pipeline, layout) = build_compute(backend, source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUCompute`] asset pipeline (`Assets<Compute>`
    /// → `ProcessedAssets<GPUCompute>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    ComputePlugin, GPUCompute
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::{BindingEntry, BindingKind};
    use crate::wgpu::test_util::with_device;

    const MINIMAL_COMPUTE_SHADER: &str = r#"
        @compute @workgroup_size(1)
        fn cs_main() {}
    "#;

    #[test]
    fn a_fragment_visible_entry_panics_before_touching_the_device() {
        with_device!(device, _queue, {
            let desc = Compute {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::sampler(ShaderStages::FRAGMENT),
                }],
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc);
            }));
            assert!(result.is_err(), "expected a panic for a non-COMPUTE-visible compute entry");
        });
    }

    #[test]
    fn a_vertex_fragment_visible_entry_also_panics() {
        // Not just "wrong stage" but "wrong stage in addition to COMPUTE" —
        // build_compute requires visibility == exactly COMPUTE, so a
        // COMPUTE | FRAGMENT entry (reused from a material by mistake, say)
        // must panic too, not just entries missing COMPUTE entirely.
        with_device!(device, _queue, {
            let desc = Compute {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(
                        ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                    ),
                }],
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE | FRAGMENT compute entry");
        });
    }

    #[test]
    fn a_compute_only_entry_builds_without_panicking() {
        with_device!(device, _queue, {
            let desc = Compute {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![],
                own_group: None,
                ..Default::default()
            };
            build_compute_raw(&device, &desc);
        });
    }
}
