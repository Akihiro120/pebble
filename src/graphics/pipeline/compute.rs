use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingKind},
            buffers::{BindGroup, Buffer, DynamicBuffer},
            cubemap::Cubemap,
            layout::{
                ComputePipelineCache, ComputePipelineKey, GlobalLayoutPool, GroupEntry, OwnEntriesBuilder, PipelineKind,
                assemble_group_layouts, find_own_entries,
            },
            params::{BindGroupParams, BindingValue, build_bind_group},
            samplers::{GlobalSamplers, SamplerKind},
            texture_array::TextureArray,
            texture_view::TextureView,
            textures::Texture,
        },
        render::Backend,
        types::flags::ShaderStages,
    },
};

pub use pebble_derive::ComputeParams;

/// A compiled GPU compute pipeline, wrapping `wgpu::ComputePipeline`. Cheap
/// to `Clone` — `wgpu::ComputePipeline` is itself an `Arc`-backed handle —
/// which is what lets [`ComputePipelineCache`] hand out a cache hit without
/// recompiling.
#[derive(Clone)]
pub struct ComputePipeline(wgpu::ComputePipeline);

impl ComputePipeline {
    pub(crate) fn raw(&self) -> &wgpu::ComputePipeline {
        &self.0
    }
}

/// A compute pipeline asset, plus the bind group values (buffers/textures)
/// it dispatches with — WGSL shader source and its bind group layout
/// compile into a `wgpu::ComputePipeline`; many `Compute`s sharing the same
/// shader automatically share one compiled pipeline (see
/// [`ComputePipelineCache`]). Dispatch it via
/// [`Backend::dispatch_compute`](crate::graphics::render::Backend::dispatch_compute).
pub struct Compute {
    label: Option<&'static str>,
    shader_source: &'static str,
    entry_point: Option<&'static str>,
    own_entries: OwnEntriesBuilder,
    extra_groups: Vec<GroupEntry>,
    params: BindGroupParams,
}

impl Default for Compute {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            entry_point: Some("cs_main"),
            own_entries: OwnEntriesBuilder::new(),
            extra_groups: Vec::new(),
            params: BindGroupParams::new(),
        }
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

    /// Declares one of this pass's own (group 0) bind group entries, at the
    /// next auto-assigned binding index — the low-level counterpart to the
    /// streamlined `.texture(...)`/`.buffer(...)`/etc. calls, for a `kind`
    /// one of those doesn't produce (a dynamic-offset buffer, a non-default
    /// sample type). Pair it with the matching value-only `.with_texture(...)`/etc.
    pub fn with_entry(mut self, name: &'static str, kind: BindingKind) -> Self {
        self.own_entries = self.own_entries.with_entry(name, kind);
        self
    }

    /// Same as [`with_entry`](Self::with_entry), pinning an explicit
    /// binding index instead of auto-assigning the next one.
    pub fn with_entry_at(mut self, name: &'static str, binding: u32, kind: BindingKind) -> Self {
        self.own_entries = self.own_entries.with_entry_at(name, binding, kind);
        self
    }

    /// Appends a bind group beyond this pass's own (group 0) — typically
    /// [`GroupEntry::Global`], a layout shared with other materials/computes
    /// via [`GlobalLayoutPool`]. Groups append in call order, starting at
    /// group 1.
    pub fn with_extra_group(mut self, group: GroupEntry) -> Self {
        self.extra_groups.push(group);
        self
    }

    /// Value-only counterpart to `.texture(...)` — see [`texture`](Self::texture).
    pub fn with_texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.params = self.params.with_texture(name, handle);
        self
    }

    /// Value-only counterpart to `.texture_array(...)` — see [`texture`](Self::texture).
    pub fn with_texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.params = self.params.with_texture_array(name, handle);
        self
    }

    /// Value-only counterpart to `.cubemap(...)` — see [`texture`](Self::texture).
    pub fn with_cubemap(mut self, name: &'static str, handle: Handle<Cubemap>) -> Self {
        self.params = self.params.with_cubemap(name, handle);
        self
    }

    /// Binds an already-built [`TextureView`] directly — e.g. one mip level
    /// from [`GPUTexture::get_view`](super::textures::GPUTexture::get_view),
    /// or a standalone render target from
    /// [`Texture::empty`](super::textures::Texture::empty). Unlike
    /// `.with_texture`/`.with_texture_array`/`.with_cubemap`, no `Handle`
    /// lookup happens at upload time — `view` must already exist.
    pub fn with_texture_view(mut self, name: &'static str, view: TextureView) -> Self {
        self.params = self.params.with_texture_view(name, view);
        self
    }

    /// Value-only counterpart to `.sampler(...)` — see [`texture`](Self::texture).
    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params = self.params.with_sampler(name, kind);
        self
    }

    /// Value-only counterpart to `.uniform(...)` — see [`texture`](Self::texture).
    pub fn with_uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params = self.params.with_uniform(name, data);
        self
    }

    /// Value-only counterpart to `.storage(...)` — see [`texture`](Self::texture).
    pub fn with_storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params = self.params.with_storage(name, data);
        self
    }

    /// Same as [`with_uniform`](Self::with_uniform), but takes a typed
    /// value instead of pre-packed bytes — uses `encase` to lay it out with
    /// correct WGSL `uniform` (std140) alignment. Value-only counterpart to
    /// `.uniform_value(...)`.
    pub fn with_uniform_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.params = self.params.with_uniform_value(name, value);
        self
    }

    /// Same as [`with_storage`](Self::with_storage), but takes a typed
    /// value instead of pre-packed bytes — uses `encase` to lay it out with
    /// correct WGSL `storage` (std430) alignment. Value-only counterpart to
    /// `.storage_value(...)`.
    pub fn with_storage_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.params = self.params.with_storage_value(name, value);
        self
    }

    /// Declares a compute-visible `texture_2d<f32>` entry at the next
    /// auto-assigned binding index *and* binds `handle` to it — the
    /// streamlined one-call form of `.with_entry(name, BindingKind::texture_2d(COMPUTE))`
    /// followed by `.with_texture(name, handle)`. Reach for those two
    /// directly for a non-default sample type or an explicit binding index
    /// — visibility is always `COMPUTE` for a compute pass, so there's no
    /// visibility to override here.
    pub fn texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_2d(ShaderStages::COMPUTE));
        self.with_texture(name, handle)
    }

    /// Streamlined form of `.texture_array(...)` — see [`texture`](Self::texture).
    pub fn texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_2d_array(ShaderStages::COMPUTE));
        self.with_texture_array(name, handle)
    }

    /// Streamlined form of `.cubemap(...)` — see [`texture`](Self::texture).
    pub fn cubemap(mut self, name: &'static str, handle: Handle<Cubemap>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_cubemap(ShaderStages::COMPUTE));
        self.with_cubemap(name, handle)
    }

    /// Streamlined form of `.sampler(...)` — see [`texture`](Self::texture).
    pub fn sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::sampler(ShaderStages::COMPUTE));
        self.with_sampler(name, kind)
    }

    /// Streamlined form of `.uniform(...)` — see [`texture`](Self::texture).
    pub fn uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::uniform_buffer(ShaderStages::COMPUTE));
        self.with_uniform(name, data)
    }

    /// Streamlined form of `.storage(...)` (read-write — a compute pass
    /// binding a storage buffer usually means to write it) — see
    /// [`texture`](Self::texture). Use `.with_entry(...)` +
    /// `.with_storage(...)` directly for a read-only one.
    pub fn storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE));
        self.with_storage(name, data)
    }

    /// Streamlined, typed form of `.uniform(...)` — declares the entry and
    /// binds an `encase`-laid-out value in one call. See [`texture`](Self::texture).
    pub fn uniform_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::uniform_buffer(ShaderStages::COMPUTE));
        self.with_uniform_value(name, value)
    }

    /// Streamlined, typed form of `.storage(...)` (read-write) — see
    /// [`storage`](Self::storage).
    pub fn storage_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE));
        self.with_storage_value(name, value)
    }

    /// Binds an existing [`Buffer`] instead of uploading raw bytes — for a
    /// buffer another pass already wrote to. Unlike `.with_uniform`/
    /// `.with_storage`, no buffer is created here; `buffer` must already
    /// carry the usage flags this binding needs.
    pub fn with_buffer(mut self, name: &'static str, buffer: Buffer) -> Self {
        self.params = self.params.with_buffer(name, buffer);
        self
    }

    /// Binds an existing [`DynamicBuffer`] — the dynamic-offset counterpart
    /// to `.with_buffer`.
    pub fn with_dynamic_buffer(mut self, name: &'static str, buffer: DynamicBuffer) -> Self {
        self.params = self.params.with_dynamic_buffer(name, buffer);
        self
    }

    pub fn with_param(mut self, name: &'static str, entry: BindingValue) -> Self {
        self.params = self.params.with_param(name, entry);
        self
    }

    /// This pass's full bind group list — its own entries (group 0, from
    /// `.texture(...)`/`.with_entry(...)`/etc.) followed by whatever
    /// `.with_extra_group(...)` appended (group 1 and up).
    fn groups(&self) -> Vec<GroupEntry> {
        std::iter::once(GroupEntry::Own(self.own_entries.entries().to_vec()))
            .chain(self.extra_groups.iter().cloned())
            .collect()
    }

    fn validate(&self) {
        if self.own_entries.entries().is_empty() && self.extra_groups.is_empty() {
            tracing::warn!(
                "Compute{}: no bind groups at all — this pass can't read or write \
                 anything; consider calling .texture(...)/.buffer(...)/etc.",
                self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            );
        }
        if self.params.is_empty() {
            tracing::warn!(
                "Compute{}: no bind group params — this pass won't bind anything against \
                 its own entries; did you forget to chain .with_texture(...)/.with_buffer(...)/etc.?",
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
/// internally by the asset upload path (behind [`ComputePipelineCache`] —
/// this always compiles, never checks the cache); exposed for callers
/// assembling pipelines outside the usual [`Assets`] flow.
pub fn build_compute(backend: &Backend, desc: &Compute, pool: &GlobalLayoutPool) -> Option<(ComputePipeline, BindGroupLayout)> {
    let groups = desc.groups();
    let own_entries = find_own_entries(desc.label, PipelineKind::Compute, &groups);
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
        &groups,
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

/// The GPU-resident form an uploaded [`Compute`] produces — its compiled
/// pipeline (possibly shared with other `Compute`s, see
/// [`ComputePipelineCache`]) plus its own bind group.
pub struct GPUCompute {
    pub pipeline: ComputePipeline,
    pub bind_group: BindGroup,
    buffers: Vec<(&'static str, Buffer)>,
    dynamic_buffers: Vec<(&'static str, DynamicBuffer)>,
}

impl GPUCompute {
    /// Overwrites a named uniform/storage buffer's contents in place —
    /// avoids rebuilding the whole bind group for a per-frame update.
    pub fn update(&self, name: &str, data: &[u8]) {
        match self.buffer(name) {
            Some(buf) => buf.write(data),
            None => tracing::warn!(
                "GPUCompute::update: no bound buffer named '{name}' — check for a typo \
                 against this pass's own .with_uniform(...)/.with_storage(...) entries"
            ),
        }
    }

    /// Same as [`update`](Self::update), but takes a typed value instead of
    /// raw bytes — same `encase` layout `Compute::with_uniform_value`/
    /// `with_storage_value` use.
    pub fn update_value<T>(&self, name: &str, value: &T)
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer
            .write(value)
            .expect("encase: failed to write value — this shouldn't happen for a #[derive(ShaderType)] struct");
        self.update(name, &buffer.into_inner());
    }

    pub fn buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }

    /// Same as [`buffer`](Self::buffer), for a binding made via
    /// `.with_dynamic_buffer` — use `DynamicBuffer::write_element` on the
    /// result to update one element in place.
    pub fn dynamic_buffer(&self, name: &str) -> Option<&DynamicBuffer> {
        self.dynamic_buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }
}

impl AssetSource for Compute {
    type Processed = GPUCompute;
}

impl Asset<Backend> for Compute {
    type Deps<'a> = (
        Read<'a, GlobalLayoutPool>,
        Read<'a, ComputePipelineCache>,
        Read<'a, Assets<Texture>>,
        Read<'a, Assets<TextureArray>>,
        Read<'a, Assets<Cubemap>>,
        Read<'a, GlobalSamplers>,
    );

    fn upload<'a>(&self, backend: &Backend, deps: &Self::Deps<'a>) -> Option<GPUCompute> {
        let (layout_pool, pipeline_cache, textures, texture_arrays, cubemaps, samplers) = deps;

        let groups = self.groups();
        let key = ComputePipelineKey::new(self.shader_source, self.entry_point, &groups);
        let (pipeline, layout) = pipeline_cache.get_or_compile(key, || build_compute(backend, self, layout_pool))?;
        let entries = find_own_entries(self.label, PipelineKind::Compute, &groups);

        let built = build_bind_group(backend, &self.params, &layout, entries, textures, texture_arrays, cubemaps, samplers)?;

        Some(GPUCompute {
            pipeline,
            bind_group: built.bind_group,
            buffers: built.buffers,
            dynamic_buffers: built.dynamic_buffers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(ComputeParams)]
    struct TestParams {
        #[storage(0)]
        data: f32,
        #[texture(1)]
        tex: Handle<Texture>,
    }

    #[test]
    fn compute_params_derive_uses_compute_visibility_and_read_write_storage() {
        let params = TestParams { data: 1.0, tex: Handle::default() };
        let compute = params.into_compute(Compute::new("shader"));

        assert!(!compute.params.is_empty());

        let groups = compute.groups();
        assert_eq!(groups.len(), 1);
        let GroupEntry::Own(entries) = &groups[0] else { panic!("group 0 should be Own") };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].binding, 0);
        assert!(entries[0].kind.visibility() == ShaderStages::COMPUTE);
        assert_eq!(entries[1].binding, 1);
        assert!(entries[1].kind.visibility() == ShaderStages::COMPUTE);
    }
}
