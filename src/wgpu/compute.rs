use crate::{
    assets::upload::Asset,
    wgpu::{backend::WGPUBackend, binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingEntry}},
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
pub struct ComputeDescriptor<'a> {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    pub label: Option<&'a str>,
    /// WGSL source for the compute stage.
    pub shader_source: &'a str,
    /// Compute stage entry point. Defaults to `"cs_main"`.
    pub entry_point: Option<&'a str>,
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

/// Builds a compute pipeline and its own bind group layout from `desc`.
///
/// Panics if any of `desc.entries` isn't visible to exactly the compute
/// stage — [`BindingKind`](super::binding::BindingKind) is shared with
/// [`MaterialDescriptor`](super::material::MaterialDescriptor), and this is
/// the check that catches a material entry (`FRAGMENT`/`VERTEX_FRAGMENT`)
/// accidentally reused in a compute pass instead of letting it fail deep
/// inside wgpu with a less specific error. The bind group layout itself
/// comes from [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder).
/// The pipeline layout is assembled from `desc.own_group` (this pass's own
/// layout) plus `desc.extra_layouts`, keyed by explicit `@group(N)` —
/// panics on a gap or a collision across `0..=max`, turning a mismatched
/// `@group(N)` in the shader into an immediate, specific error instead of
/// an opaque wgpu validation failure at draw time.
pub fn build_compute(
    device: &wgpu::Device,
    desc: &ComputeDescriptor,
) -> (ComputePipeline, BindGroupLayout) {
    for entry in &desc.entries {
        if entry.kind.visibility() != wgpu::ShaderStages::COMPUTE {
            panic!(
                "compute pass{}: entry '{}' has visibility {:?} — compute bind group entries \
                 must be visible to exactly the compute stage",
                desc.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                entry.name,
                entry.kind.visibility()
            );
        }
    }

    let layout = BindGroupLayoutBuilder::new()
        .label(desc.label)
        .entries(desc.entries.iter().cloned())
        .build(device);

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

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUCompute`] asset pipeline (`Assets<ComputeDescriptor>`
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
            let desc = ComputeDescriptor {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::sampler(wgpu::ShaderStages::FRAGMENT),
                }],
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute(&device, &desc);
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
            let desc = ComputeDescriptor {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(
                        wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ),
                }],
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute(&device, &desc);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE | FRAGMENT compute entry");
        });
    }

    #[test]
    fn a_compute_only_entry_builds_without_panicking() {
        with_device!(device, _queue, {
            let desc = ComputeDescriptor {
                shader_source: MINIMAL_COMPUTE_SHADER,
                entries: vec![],
                own_group: None,
                ..Default::default()
            };
            build_compute(&device, &desc);
        });
    }
}
