use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingKind},
            buffers::{BindGroup, Buffer, DynamicBuffer},
            cubemap::Cubemap,
            layout::{
                GlobalLayoutPool, GroupEntry, MaterialPipelineCache, MaterialPipelineKey, OwnEntriesBuilder, PipelineKind,
                assemble_group_layouts, find_own_entries,
            },
            params::{BindGroupParams, BindingValue, build_bind_group},
            samplers::{GlobalSamplers, SamplerKind},
            texture_array::TextureArray,
            texture_view::TextureView,
            textures::Texture,
        },
        render::Backend,
        types::{
            Face, PolygonMode,
            flags::ShaderStages,
            pipeline_state::{ColorTargetState, DepthStencilState, VertexBufferLayout},
        },
    },
};

use super::mesh::Vertex;

pub use pebble_derive::MaterialParams;

/// A compiled GPU render pipeline, wrapping `wgpu::RenderPipeline`. Cheap to
/// `Clone` — `wgpu::RenderPipeline` is itself an `Arc`-backed handle — which
/// is what lets [`MaterialPipelineCache`] hand out a cache hit without
/// recompiling.
#[derive(Clone)]
pub struct RenderPipeline(wgpu::RenderPipeline);

impl RenderPipeline {
    pub(crate) fn raw(&self) -> &wgpu::RenderPipeline {
        &self.0
    }
}

/// A material's color targets — either an explicit list, or (for
/// [`Material::standard`]) a marker resolved against the real surface
/// format at upload time, once `Backend` is available — same idea as
/// [`MipLevels`](super::mipmap::MipLevels) resolving a mip count only once
/// a texture's actual size is known.
enum TargetsSpec {
    Explicit(Vec<ColorTargetState>),
    SurfaceDefault,
}

impl TargetsSpec {
    fn len(&self) -> usize {
        match self {
            Self::Explicit(targets) => targets.len(),
            Self::SurfaceDefault => 1,
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, Self::Explicit(targets) if targets.is_empty())
    }

    fn resolve(&self, backend: &Backend) -> Vec<ColorTargetState> {
        match self {
            Self::Explicit(targets) => targets.clone(),
            Self::SurfaceDefault => {
                vec![ColorTargetState { format: backend.surface_format(), ..Default::default() }]
            }
        }
    }
}

/// A render pipeline asset, plus the bind group values (textures/samplers/
/// uniforms/storage buffers) it renders with — WGSL shader source and
/// fixed-function state (vertex layouts, cull mode, depth, targets) compile
/// into a `wgpu::RenderPipeline`; many `Material`s sharing the same shader
/// and fixed-function state automatically share one compiled pipeline (see
/// [`MaterialPipelineCache`]), so "the same shader, several different
/// uniform-value combinations" is just several `Material`s, not a separate
/// instance concept.
///
/// Bind group 0 is this material's own — build it with the streamlined
/// per-binding calls (`.texture(...)`/`.sampler(...)`/`.uniform_value(...)`/etc.,
/// each declaring the entry *and* providing its value in one call, visible
/// to the fragment stage, binding index auto-assigned) for the common case,
/// or `.with_entry(...)`/`.with_entry_at(...)` plus the matching `.with_texture(...)`/etc.
/// value-only call when you need to override visibility, sample type, or the
/// binding index. `.with_extra_group(...)` appends group 1 and up — a
/// shared/global layout, most often.
pub struct Material {
    label: Option<&'static str>,
    shader_source: &'static str,
    vertex_entry: Option<&'static str>,
    fragment_entry: Option<&'static str>,
    vertex_layouts: Vec<VertexBufferLayout>,
    own_entries: OwnEntriesBuilder,
    extra_groups: Vec<GroupEntry>,
    cull_mode: Option<Face>,
    depth: Option<DepthStencilState>,
    targets: TargetsSpec,
    polygon_mode: PolygonMode,
    sample_count: u32,
    params: BindGroupParams,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: Vec::new(),
            own_entries: OwnEntriesBuilder::new(),
            extra_groups: Vec::new(),
            cull_mode: Some(Face::default()),
            depth: None,
            targets: TargetsSpec::Explicit(Vec::new()),
            polygon_mode: PolygonMode::default(),
            sample_count: 1,
            params: BindGroupParams::new(),
        }
    }
}

impl Material {
    pub fn new(shader_source: &'static str) -> Self {
        Self {
            shader_source,
            ..Self::default()
        }
    }

    /// Like `new`, but pre-filled with the common opaque-3D-geometry
    /// defaults instead of leaving them empty: `Vertex::layout()` for
    /// `.with_vertex_layouts`, a single opaque target in the real surface
    /// format for `.with_targets` (rendering straight to the screen is the
    /// common case this saves you from getting wrong — the surface format
    /// varies by platform/backend, e.g. `Bgra8Unorm` is common on
    /// Windows/DX12, not the `Rgba8Unorm` [`DEFAULT_TARGET`] assumes), and
    /// [`DepthStencilState::DEFAULT`] for `.with_depth`. The surface format
    /// is resolved against `Backend` at upload time, not here — `standard`
    /// itself needs no `Backend` reference, same as `new`. Still a plain
    /// builder — chain `.with_vertex_layouts(...)`/`.with_targets(...)`/
    /// `.with_depth(...)`/`.without_depth()`/etc. afterwards to override
    /// any of these for a material that doesn't fit the common case (a
    /// custom vertex type, an offscreen target with a different/blended
    /// format, no depth test).
    pub fn standard(shader_source: &'static str) -> Self {
        let mut material = Self::new(shader_source)
            .with_vertex_layouts(vec![Vertex::layout()])
            .with_depth(DepthStencilState::DEFAULT);
        material.targets = TargetsSpec::SurfaceDefault;
        material
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

    /// Declares one of this material's own (group 0) bind group entries,
    /// at the next auto-assigned binding index — the low-level counterpart
    /// to the streamlined `.texture(...)`/`.sampler(...)`/etc. calls, for
    /// when you need a `kind` one of those doesn't produce (vertex/compute
    /// visibility, a non-default sample type, a dynamic-offset buffer).
    /// Pair it with the matching value-only `.with_texture(...)`/`.with_sampler(...)`/etc.
    /// call.
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

    /// Appends a bind group beyond this material's own (group 0) —
    /// typically [`GroupEntry::Global`], a layout shared with other
    /// materials/computes via [`GlobalLayoutPool`]. Groups append in call
    /// order, starting at group 1.
    pub fn with_extra_group(mut self, group: GroupEntry) -> Self {
        self.extra_groups.push(group);
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

    pub fn without_depth(mut self) -> Self {
        self.depth = None;
        self
    }

    pub fn with_targets(mut self, targets: Vec<ColorTargetState>) -> Self {
        self.targets = TargetsSpec::Explicit(targets);
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

    /// Binds a texture value against an entry declared separately (via
    /// `.with_entry(...)`/`.with_entry_at(...)`) — for when `.texture(...)`'s
    /// fragment-visible/filterable-float default isn't right. Most
    /// materials want `.texture(...)` instead.
    pub fn with_texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.params = self.params.with_texture(name, handle);
        self
    }

    /// Value-only counterpart to `.texture_array(...)` — see `.with_texture`.
    pub fn with_texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.params = self.params.with_texture_array(name, handle);
        self
    }

    /// Value-only counterpart to `.cubemap(...)` — see `.with_texture`.
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

    /// Value-only counterpart to `.sampler(...)` — see `.with_texture`.
    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params = self.params.with_sampler(name, kind);
        self
    }

    /// Value-only counterpart to `.uniform(...)` — see `.with_texture`.
    pub fn with_uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params = self.params.with_uniform(name, data);
        self
    }

    /// Value-only counterpart to `.storage(...)` — see `.with_texture`.
    /// Declares a read-write entry via `.with_entry(...)` if you need one;
    /// `.storage(...)`'s streamlined default is read-only.
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

    /// Declares a fragment-visible `texture_2d<f32>` entry at the next
    /// auto-assigned binding index *and* binds `handle` to it — the
    /// streamlined one-call form of `.with_entry(name, BindingKind::texture_2d(FRAGMENT))`
    /// followed by `.with_texture(name, handle)`. Reach for those two
    /// directly instead when you need vertex/compute visibility, a
    /// non-default sample type, or an explicit binding index.
    pub fn texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_2d(ShaderStages::FRAGMENT));
        self.with_texture(name, handle)
    }

    /// Streamlined form of `.texture_array(...)` — see [`texture`](Self::texture).
    pub fn texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_2d_array(ShaderStages::FRAGMENT));
        self.with_texture_array(name, handle)
    }

    /// Streamlined form of `.cubemap(...)` — see [`texture`](Self::texture).
    pub fn cubemap(mut self, name: &'static str, handle: Handle<Cubemap>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::texture_cubemap(ShaderStages::FRAGMENT));
        self.with_cubemap(name, handle)
    }

    /// Streamlined form of `.sampler(...)` — see [`texture`](Self::texture).
    pub fn sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::sampler(ShaderStages::FRAGMENT));
        self.with_sampler(name, kind)
    }

    /// Streamlined form of `.uniform(...)` — see [`texture`](Self::texture).
    pub fn uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::uniform_buffer(ShaderStages::FRAGMENT));
        self.with_uniform(name, data)
    }

    /// Streamlined form of `.storage(...)` (read-only) — see [`texture`](Self::texture).
    pub fn storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::storage_buffer_read_only(ShaderStages::FRAGMENT));
        self.with_storage(name, data)
    }

    /// Streamlined, typed form of `.uniform(...)` — declares the entry and
    /// binds an `encase`-laid-out value in one call. See [`texture`](Self::texture).
    pub fn uniform_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::uniform_buffer(ShaderStages::FRAGMENT));
        self.with_uniform_value(name, value)
    }

    /// Streamlined, typed form of `.storage(...)` (read-only) — see [`texture`](Self::texture).
    pub fn storage_value<T>(mut self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        self.own_entries = self.own_entries.with_entry(name, BindingKind::storage_buffer_read_only(ShaderStages::FRAGMENT));
        self.with_storage_value(name, value)
    }

    /// Binds an existing [`Buffer`] instead of uploading raw bytes — for a
    /// buffer you already built yourself (e.g. one a compute pass writes
    /// to, then this material reads from). Unlike `.with_uniform`/`.with_storage`,
    /// no buffer is created here; `buffer` must already carry the usage
    /// flags this binding needs.
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

    /// This material's full bind group list — its own entries (group 0,
    /// from `.texture(...)`/`.with_entry(...)`/etc.) followed by whatever
    /// `.with_extra_group(...)` appended (group 1 and up).
    fn groups(&self) -> Vec<GroupEntry> {
        std::iter::once(GroupEntry::Own(self.own_entries.entries().to_vec()))
            .chain(self.extra_groups.iter().cloned())
            .collect()
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
        if self.params.is_empty() {
            tracing::warn!(
                "Material{}: no bind group params — this material won't bind anything against \
                 its own entries; did you forget to chain .with_texture(...)/.with_sampler(...)/etc.?",
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

    let attribute_count: u32 = desc
        .vertex_layouts
        .iter()
        .map(|l| l.attributes.len() as u32)
        .sum();
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

/// Compiles a [`Material`] into a raw pipeline + bind group layout. Used
/// internally by the asset upload path (behind [`MaterialPipelineCache`] —
/// this always compiles, never checks the cache); exposed for callers
/// building their own asset wiring around a `Material` outside the usual
/// [`Assets`] flow.
pub fn build_material(
    backend: &Backend,
    desc: &Material,
    pool: &GlobalLayoutPool,
) -> Option<(RenderPipeline, BindGroupLayout)> {
    check_material_limits(&backend.device, desc);

    let groups = desc.groups();
    let own_entries = find_own_entries(desc.label, PipelineKind::Material, &groups);
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

    let attribute_sets: Vec<Vec<wgpu::VertexAttribute>> = desc
        .vertex_layouts
        .iter()
        .map(|l| l.attributes.iter().map(|a| (*a).into()).collect())
        .collect();
    let vertex_buffers: Vec<Option<wgpu::VertexBufferLayout>> = desc
        .vertex_layouts
        .iter()
        .zip(attribute_sets.iter())
        .map(|(l, attrs)| {
            Some(wgpu::VertexBufferLayout {
                array_stride: l.array_stride,
                step_mode: l.step_mode.into(),
                attributes: attrs,
            })
        })
        .collect();

    let targets: Vec<Option<wgpu::ColorTargetState>> =
        desc.targets.resolve(backend).into_iter().map(|t| Some(t.into())).collect();

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

/// The GPU-resident form an uploaded [`Material`] produces — its compiled
/// pipeline (possibly shared with other `Material`s, see
/// [`MaterialPipelineCache`]) plus its own bind group.
pub struct GPUMaterial {
    pub pipeline: RenderPipeline,
    pub bind_group: BindGroup,
    buffers: Vec<(&'static str, Buffer)>,
    dynamic_buffers: Vec<(&'static str, DynamicBuffer)>,
}

impl GPUMaterial {
    /// Overwrites a named uniform/storage buffer's contents in place —
    /// avoids rebuilding the whole bind group for a per-frame update.
    pub fn update(&self, name: &str, data: &[u8]) {
        match self.buffer(name) {
            Some(buf) => buf.write(data),
            None => tracing::warn!(
                "GPUMaterial::update: no bound buffer named '{name}' — check for a typo \
                 against this material's own .with_uniform(...)/.with_storage(...) entries"
            ),
        }
    }

    /// Same as [`update`](Self::update), but takes a typed value instead of
    /// raw bytes — same `encase` layout `Material::with_uniform_value`/
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

impl AssetSource for Material {
    type Processed = GPUMaterial;
}

impl Asset<Backend> for Material {
    type Deps<'a> = (
        Read<'a, GlobalLayoutPool>,
        Read<'a, MaterialPipelineCache>,
        Read<'a, Assets<Texture>>,
        Read<'a, Assets<TextureArray>>,
        Read<'a, Assets<Cubemap>>,
        Read<'a, GlobalSamplers>,
    );

    fn upload<'a>(&self, backend: &Backend, deps: &Self::Deps<'a>) -> Option<GPUMaterial> {
        let (layout_pool, pipeline_cache, textures, texture_arrays, cubemaps, samplers) = deps;

        let groups = self.groups();
        let key = MaterialPipelineKey::new(
            self.shader_source,
            self.vertex_entry,
            self.fragment_entry,
            self.vertex_layouts.clone(),
            self.cull_mode,
            self.depth.clone(),
            self.targets.resolve(backend),
            self.polygon_mode,
            self.sample_count,
            &groups,
        );
        let (pipeline, layout) = pipeline_cache.get_or_compile(key, || build_material(backend, self, layout_pool))?;
        let entries = find_own_entries(self.label, PipelineKind::Material, &groups);

        let built = build_bind_group(backend, &self.params, &layout, entries, textures, texture_arrays, cubemaps, samplers)?;

        Some(GPUMaterial {
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

    #[derive(MaterialParams)]
    #[layout("shared_camera")]
    struct TestParams {
        #[uniform(0)]
        a: f32,
        #[uniform(0)]
        b: f32,
        #[texture(1)]
        tex: Handle<Texture>,
        #[sampler(2)]
        samp: SamplerKind,
    }

    #[test]
    fn material_params_derive_groups_shared_index_and_auto_appends_global_layout() {
        let params = TestParams { a: 1.0, b: 2.0, tex: Handle::default(), samp: SamplerKind::LinearRepeat };
        let material = params.into_material(Material::new("shader"));

        assert!(!material.params.is_empty());

        let groups = material.groups();
        assert_eq!(groups.len(), 2, "own group 0 + the #[layout(\"shared_camera\")] extra group");

        let GroupEntry::Own(entries) = &groups[0] else { panic!("group 0 should be Own") };
        // one combined entry for `a`+`b` (shared binding 0), plus texture (1), sampler (2)
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].binding, 0);
        assert_eq!(entries[0].name, "a"); // first field in the group names it
        assert_eq!(entries[1].binding, 1);
        assert_eq!(entries[1].name, "tex");
        assert_eq!(entries[2].binding, 2);
        assert_eq!(entries[2].name, "samp");

        match &groups[1] {
            GroupEntry::Global(name) => assert_eq!(*name, "shared_camera"),
            _ => panic!("group 1 should be the #[layout(\"shared_camera\")] Global entry"),
        }
    }

    #[derive(MaterialParams)]
    #[layout(param)]
    struct TestParamsWithParamLayout {
        #[texture(0)]
        tex: Handle<Texture>,
    }

    #[test]
    fn material_params_derive_with_param_layout_takes_caller_supplied_group() {
        let params = TestParamsWithParamLayout { tex: Handle::default() };
        // `#[layout(param)]` means `into_material` takes the extra group as
        // an argument (`extra_group_0`) instead of baking in a fixed name.
        let material = params.into_material(Material::new("shader"), GroupEntry::Global("lighting"));

        let groups = material.groups();
        assert_eq!(groups.len(), 2);
        match &groups[1] {
            GroupEntry::Global(name) => assert_eq!(*name, "lighting"),
            _ => panic!("group 1 should be the caller-supplied GroupEntry"),
        }
    }

    #[derive(MaterialParams)]
    struct TestOptionalTexture {
        #[texture(0, vertex)]
        tex: Option<Handle<Texture>>,
    }

    #[test]
    fn material_params_derive_optional_texture_uses_fallback_and_visibility_override() {
        let fallback = Handle::<Texture>::default();
        let with_none = TestOptionalTexture { tex: None }.into_material(Material::new("shader"), fallback);
        let with_some = TestOptionalTexture { tex: Some(Handle::default()) }.into_material(Material::new("shader"), fallback);

        for material in [with_none, with_some] {
            assert!(!material.params.is_empty());
            let groups = material.groups();
            let GroupEntry::Own(entries) = &groups[0] else { panic!("expected Own group") };
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].binding, 0);
            assert!(entries[0].kind.visibility() == ShaderStages::VERTEX);
        }
    }

    #[derive(MaterialParams)]
    #[layout("camera")]
    #[layout(param)]
    #[layout(param)]
    struct TestMultipleExtraGroups {
        #[texture(0)]
        tex: Handle<Texture>,
    }

    #[test]
    fn material_params_derive_supports_multiple_param_layouts_in_declaration_order() {
        // GroupEntry::Layout(..) needs a real &Backend to build (not available
        // in a unit test) — Global stands in here for "some GroupEntry value
        // the caller supplies," since the two `param` slots are typed as the
        // full GroupEntry enum and don't care which variant arrives.
        let params = TestMultipleExtraGroups { tex: Handle::default() };
        let material = params.into_material(
            Material::new("shader"),
            GroupEntry::Global("first_custom"),
            GroupEntry::Global("second_custom"),
        );

        let groups = material.groups();
        assert_eq!(groups.len(), 4, "own group 0 + camera + two param layouts");
        assert!(matches!(&groups[1], GroupEntry::Global(name) if *name == "camera"));
        assert!(matches!(&groups[2], GroupEntry::Global(name) if *name == "first_custom"));
        assert!(matches!(&groups[3], GroupEntry::Global(name) if *name == "second_custom"));
    }
}
