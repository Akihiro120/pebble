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
/// [`GPUCompute`] is built from via [`build_compute`]. Fields are private —
/// the only way to construct one is [`ComputeBuilder`]:
/// `ComputeBuilder::new(shader_source).build()`.
pub struct Compute {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    label: Option<&'static str>,
    /// WGSL source for the compute stage.
    shader_source: &'static str,
    /// Compute stage entry point. Defaults to `"cs_main"`.
    entry_point: Option<&'static str>,
    /// This compute pass's bind groups, in `@group(N)` order — set via
    /// [`entries`](ComputeBuilder::entries), whose docs cover the full shape.
    groups: Vec<super::layout::GroupEntry>,
}

/// Builds a [`Compute`]. Start from [`new`](Self::new), chain the setters
/// below, then finish with [`build`](Self::build)/[`build_asset`](Self::build_asset).
pub struct ComputeBuilder {
    label: Option<&'static str>,
    shader_source: &'static str,
    entry_point: Option<&'static str>,
    groups: Vec<super::layout::GroupEntry>,
}

impl Default for ComputeBuilder {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            entry_point: Some("cs_main"),
            groups: Vec::new(),
        }
    }
}

impl ComputeBuilder {
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

    /// This compute pass's bind groups, in `@group(N)` order — position in `groups` *is* the
    /// `@group(N)` index a shader must declare to match: the first element is `@group(0)`,
    /// the second `@group(1)`, and so on. Each element is either:
    ///
    /// - [`GroupEntry::Own`](super::layout::GroupEntry::Own) — this compute pass's own bind
    ///   group entries, built into a fresh layout internally. At most one of these is
    ///   allowed — the one group a
    ///   [`GPUComputeInstance`](super::instance::GPUComputeInstance) binds concrete resources
    ///   against — `build_compute` panics on a second one.
    /// - [`GroupEntry::Layout`](super::layout::GroupEntry::Layout) — an already-built layout
    ///   occupying this position directly: any external bind group layout, e.g. pulled from a
    ///   [`GlobalLayoutPool`](super::layout::GlobalLayoutPool) via
    ///   [`GlobalLayoutPool::get`](super::layout::GlobalLayoutPool::get).
    ///
    /// `build_compute` also panics if any `Own` entry isn't visible to exactly the compute
    /// stage, or if `groups` needs more bind groups than the device's `max_bind_groups`
    /// allows (`wgpu` guarantees only 4) — list only the groups this pass's shader actually
    /// declares.
    pub fn entries(mut self, groups: Vec<super::layout::GroupEntry>) -> Self {
        self.groups = groups;
        self
    }

    /// Logs a WARN if this pass has no bind groups at all — not fatal, since a shader could
    /// legitimately need no bindings, but a compute pass with nothing to read or write is
    /// unusual enough to flag.
    fn validate(&self) {
        if self.groups.is_empty() {
            tracing::warn!(
                "ComputeBuilder{}: no bind groups at all — this pass can't read or write \
                 anything; consider calling .entries(...)",
                self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            );
        }
    }

    /// Consume the builder and return the finished [`Compute`] value.
    pub fn build(self) -> Compute {
        self.validate();
        Compute {
            label: self.label,
            shader_source: self.shader_source,
            entry_point: self.entry_point,
            groups: self.groups,
        }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Compute>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Compute>) -> Handle<Compute> {
        let compute = self.build();
        assets.insert(name, compute)
    }
}

/// Builds a compute pipeline and its own bind group layout from `desc`.
///
/// Panics if the one [`GroupEntry::Own`](super::layout::GroupEntry::Own) in `desc.entries`
/// (if any) isn't visible to exactly the compute stage —
/// [`BindingKind`](super::binding::BindingKind) is shared with
/// [`Material`](super::material::Material), and this is the check that catches a material
/// entry (`FRAGMENT`/`VERTEX_FRAGMENT`) accidentally reused in a compute pass instead of
/// letting it fail deep inside wgpu with a less specific error. The bind group layout itself
/// comes from [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder). The
/// pipeline layout is assembled directly from `desc.entries`, in order — position is the
/// `@group(N)` index — panicking if `desc.entries` contains more than one `GroupEntry::Own`,
/// or needs more bind groups than the device's `max_bind_groups` allows, turning either
/// mistake into an immediate, specific error instead of an opaque wgpu validation failure at
/// draw time.
///
/// Returns `None` — not a panic — if `desc.entries` contains a
/// [`GroupEntry::Global`](super::layout::GroupEntry::Global) not yet registered in `pool`; the
/// caller (`GPUCompute::upload`) treats that exactly like any other unmet `Deps` and retries
/// next tick.
pub fn build_compute(
    backend: &WGPUBackend,
    desc: &Compute,
    pool: &super::layout::GlobalLayoutPool,
) -> Option<(ComputePipeline, BindGroupLayout)> {
    build_compute_raw(&backend.device, desc, pool)
}

/// Internal primitive behind [`build_compute`] — used directly only by
/// tests, which have a raw `wgpu::Device` but no full [`WGPUBackend`].
pub(crate) fn build_compute_raw(
    device: &wgpu::Device,
    desc: &Compute,
    pool: &super::layout::GlobalLayoutPool,
) -> Option<(ComputePipeline, BindGroupLayout)> {
    let own_entries =
        super::layout::find_own_entries(desc.label, super::layout::PipelineKind::Compute, &desc.groups);
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
        .label(desc.label)
        .entries(own_entries.iter().cloned())
        .build_raw(device);

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: desc.label,
        source: wgpu::ShaderSource::Wgsl(desc.shader_source.into()),
    });

    let bind_group_layouts = super::layout::assemble_group_layouts(
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
    type Deps<'a> = crate::ecs::system::Res<'a, super::layout::GlobalLayoutPool>;

    fn upload<'a>(
        source: &Compute,
        backend: &WGPUBackend,
        pool: &crate::ecs::system::Res<'a, super::layout::GlobalLayoutPool>,
    ) -> Option<Self> {
        let (pipeline, layout) = build_compute(backend, source, pool)?;
        let entries =
            super::layout::find_own_entries(source.label, super::layout::PipelineKind::Compute, &source.groups)
                .to_vec();

        Some(Self { pipeline, layout, entries })
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
    fn a_fragment_visible_own_entry_panics_before_touching_the_device() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![super::super::layout::GroupEntry::Own(vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::sampler(ShaderStages::FRAGMENT),
                }])])
                .build();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for a non-COMPUTE-visible compute entry");
        });
    }

    #[test]
    fn a_vertex_fragment_visible_own_entry_also_panics() {
        // Not just "wrong stage" but "wrong stage in addition to COMPUTE" —
        // build_compute requires visibility == exactly COMPUTE, so a
        // COMPUTE | FRAGMENT entry (reused from a material by mistake, say)
        // must panic too, not just entries missing COMPUTE entirely.
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![super::super::layout::GroupEntry::Own(vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(
                        ShaderStages::COMPUTE | ShaderStages::FRAGMENT,
                    ),
                }])])
                .build();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE | FRAGMENT compute entry");
        });
    }

    #[test]
    fn no_entries_at_all_builds_without_panicking() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER).build();
            build_compute_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_layout_pulled_from_the_global_pool_ends_up_in_the_pipeline_layout() {
        with_device!(device, _queue, {
            let mut pool = super::super::layout::GlobalLayoutPool::new();
            pool.register("camera", crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device));

            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![super::super::layout::GroupEntry::Layout(pool.get("camera").unwrap())])
                .build();

            build_compute_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_global_entry_resolves_from_the_pool_at_build_time() {
        with_device!(device, _queue, {
            let mut pool = super::super::layout::GlobalLayoutPool::new();
            pool.register("camera", crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device));

            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![super::super::layout::GroupEntry::Global("camera")])
                .build();

            build_compute_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_global_entry_not_yet_registered_returns_none_instead_of_panicking() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new(); // "camera" never registered
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![super::super::layout::GroupEntry::Global("camera")])
                .build();

            assert!(build_compute_raw(&device, &desc, &pool).is_none());
        });
    }

    #[test]
    fn own_and_layout_groups_are_ordered_by_position() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let extra = crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device);
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![
                    super::super::layout::GroupEntry::Own(vec![]),
                    super::super::layout::GroupEntry::Layout(extra),
                ])
                .build();

            build_compute_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn more_than_one_own_group_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER)
                .entries(vec![
                    super::super::layout::GroupEntry::Own(vec![]),
                    super::super::layout::GroupEntry::Own(vec![]),
                ])
                .build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for more than one Own group");
        });
    }

    #[test]
    fn exceeding_max_bind_groups_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            // This device's real max_bind_groups is at least 4, so 5 groups always exceeds it.
            let groups: Vec<super::super::layout::GroupEntry> = (0..5)
                .map(|_| {
                    super::super::layout::GroupEntry::Layout(
                        crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device),
                    )
                })
                .collect();
            let desc = ComputeBuilder::new(MINIMAL_COMPUTE_SHADER).entries(groups).build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_compute_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_bind_groups");
        });
    }
}
