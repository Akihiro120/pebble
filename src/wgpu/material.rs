use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    wgpu::{
        backend::WGPUBackend,
        binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingEntry},
        flags::ShaderStages,
        texture_format::TextureFormat,
        vertex_format::VertexBufferLayout,
    },
};

/// A `wgpu::RenderPipeline`, opaque — built only via [`build_material`]/
/// [`GPUMaterial`]'s `Asset::upload`. Bind it against a
/// [`RenderPass`](super::render_pass::RenderPass) via
/// [`RenderPass::set_pipeline`](super::render_pass::RenderPass::set_pipeline);
/// there's no way to reach the underlying `wgpu::RenderPipeline` from
/// outside this crate.
pub struct RenderPipeline(wgpu::RenderPipeline);

impl RenderPipeline {
    pub(crate) fn raw(&self) -> &wgpu::RenderPipeline {
        &self.0
    }
}

/// Face of a vertex considered for culling — mirrors `wgpu::Face`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum Face {
    Front,
    Back,
}

impl From<Face> for wgpu::Face {
    fn from(value: Face) -> Self {
        match value {
            Face::Front => Self::Front,
            Face::Back => Self::Back,
        }
    }
}

/// Rasterizer polygon mode — mirrors `wgpu::PolygonMode`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum PolygonMode {
    Fill,
    Line,
    Point,
}

impl From<PolygonMode> for wgpu::PolygonMode {
    fn from(value: PolygonMode) -> Self {
        match value {
            PolygonMode::Fill => Self::Fill,
            PolygonMode::Line => Self::Line,
            PolygonMode::Point => Self::Point,
        }
    }
}

/// Color/alpha blend factor — mirrors `wgpu::BlendFactor`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
    Src1,
    OneMinusSrc1,
    Src1Alpha,
    OneMinusSrc1Alpha,
}

impl From<BlendFactor> for wgpu::BlendFactor {
    fn from(value: BlendFactor) -> Self {
        match value {
            BlendFactor::Zero => Self::Zero,
            BlendFactor::One => Self::One,
            BlendFactor::Src => Self::Src,
            BlendFactor::OneMinusSrc => Self::OneMinusSrc,
            BlendFactor::SrcAlpha => Self::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => Self::OneMinusSrcAlpha,
            BlendFactor::Dst => Self::Dst,
            BlendFactor::OneMinusDst => Self::OneMinusDst,
            BlendFactor::DstAlpha => Self::DstAlpha,
            BlendFactor::OneMinusDstAlpha => Self::OneMinusDstAlpha,
            BlendFactor::SrcAlphaSaturated => Self::SrcAlphaSaturated,
            BlendFactor::Constant => Self::Constant,
            BlendFactor::OneMinusConstant => Self::OneMinusConstant,
            BlendFactor::Src1 => Self::Src1,
            BlendFactor::OneMinusSrc1 => Self::OneMinusSrc1,
            BlendFactor::Src1Alpha => Self::Src1Alpha,
            BlendFactor::OneMinusSrc1Alpha => Self::OneMinusSrc1Alpha,
        }
    }
}

/// Color/alpha blend operation — mirrors `wgpu::BlendOperation`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl From<BlendOperation> for wgpu::BlendOperation {
    fn from(value: BlendOperation) -> Self {
        match value {
            BlendOperation::Add => Self::Add,
            BlendOperation::Subtract => Self::Subtract,
            BlendOperation::ReverseSubtract => Self::ReverseSubtract,
            BlendOperation::Min => Self::Min,
            BlendOperation::Max => Self::Max,
        }
    }
}

/// One color or alpha blend equation — mirrors `wgpu::BlendComponent`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

impl BlendComponent {
    /// Replaces the destination with the source outright.
    pub const REPLACE: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    };

    /// `(1 * src) + ((1 - src_alpha) * dst)`.
    pub const OVER: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    };
}

impl From<BlendComponent> for wgpu::BlendComponent {
    fn from(value: BlendComponent) -> Self {
        Self {
            src_factor: value.src_factor.into(),
            dst_factor: value.dst_factor.into(),
            operation: value.operation.into(),
        }
    }
}

/// Blend state of a color target — mirrors `wgpu::BlendState`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

impl BlendState {
    /// No color blending — overwrites the target with the shader's output.
    pub const REPLACE: Self = Self { color: BlendComponent::REPLACE, alpha: BlendComponent::REPLACE };

    /// Standard alpha blending with non-premultiplied alpha.
    pub const ALPHA_BLENDING: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent::OVER,
    };

    /// Standard alpha blending with premultiplied alpha.
    pub const PREMULTIPLIED_ALPHA_BLENDING: Self =
        Self { color: BlendComponent::OVER, alpha: BlendComponent::OVER };
}

impl From<BlendState> for wgpu::BlendState {
    fn from(value: BlendState) -> Self {
        Self { color: value.color.into(), alpha: value.alpha.into() }
    }
}

/// Describes the color state of a render pipeline — mirrors
/// `wgpu::ColorTargetState`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ColorTargetState {
    /// The format of the attachment this pipeline renders to.
    pub format: TextureFormat,
    /// Blending used for this target. `None` disables blending.
    pub blend: Option<BlendState>,
    /// Which color/alpha channels get written.
    pub write_mask: super::flags::ColorWrites,
}

impl From<ColorTargetState> for wgpu::ColorTargetState {
    fn from(value: ColorTargetState) -> Self {
        Self {
            format: value.format.into(),
            blend: value.blend.map(Into::into),
            write_mask: value.write_mask.into(),
        }
    }
}

/// A single opaque `Rgba8Unorm` color target with no blending — a
/// ready-made value for [`MaterialBuilder::targets`] when you don't need
/// anything more specific. Not applied automatically by `Default` (which
/// leaves `targets` empty, since the right format usually depends on the
/// surface/render target), so use it explicitly: `targets:
/// DEFAULT_TARGET.to_vec()`.
pub const DEFAULT_TARGET: [ColorTargetState; 1] = [ColorTargetState {
    format: TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: super::flags::ColorWrites::ALL,
}];

/// Comparison function used for depth/stencil operations — mirrors
/// `wgpu::CompareFunction`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl From<CompareFunction> for wgpu::CompareFunction {
    fn from(value: CompareFunction) -> Self {
        match value {
            CompareFunction::Never => Self::Never,
            CompareFunction::Less => Self::Less,
            CompareFunction::Equal => Self::Equal,
            CompareFunction::LessEqual => Self::LessEqual,
            CompareFunction::Greater => Self::Greater,
            CompareFunction::NotEqual => Self::NotEqual,
            CompareFunction::GreaterEqual => Self::GreaterEqual,
            CompareFunction::Always => Self::Always,
        }
    }
}

/// Operation performed on the stencil value — mirrors `wgpu::StencilOperation`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
}

impl From<StencilOperation> for wgpu::StencilOperation {
    fn from(value: StencilOperation) -> Self {
        match value {
            StencilOperation::Keep => Self::Keep,
            StencilOperation::Zero => Self::Zero,
            StencilOperation::Replace => Self::Replace,
            StencilOperation::Invert => Self::Invert,
            StencilOperation::IncrementClamp => Self::IncrementClamp,
            StencilOperation::DecrementClamp => Self::DecrementClamp,
            StencilOperation::IncrementWrap => Self::IncrementWrap,
            StencilOperation::DecrementWrap => Self::DecrementWrap,
        }
    }
}

/// Per-face stencil test/operation state — mirrors `wgpu::StencilFaceState`.
/// If you're not using stencil testing, leave this as [`Self::IGNORE`]
/// (the [`Default`]).
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct StencilFaceState {
    pub compare: CompareFunction,
    pub fail_op: StencilOperation,
    pub depth_fail_op: StencilOperation,
    pub pass_op: StencilOperation,
}

impl StencilFaceState {
    pub const IGNORE: Self = Self {
        compare: CompareFunction::Always,
        fail_op: StencilOperation::Keep,
        depth_fail_op: StencilOperation::Keep,
        pass_op: StencilOperation::Keep,
    };
}

impl Default for StencilFaceState {
    fn default() -> Self {
        Self::IGNORE
    }
}

impl From<StencilFaceState> for wgpu::StencilFaceState {
    fn from(value: StencilFaceState) -> Self {
        Self {
            compare: value.compare.into(),
            fail_op: value.fail_op.into(),
            depth_fail_op: value.depth_fail_op.into(),
            pass_op: value.pass_op.into(),
        }
    }
}

/// Full stencil test state — mirrors `wgpu::StencilState`. Defaults to
/// disabled (both faces [`StencilFaceState::IGNORE`], zero masks).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct StencilState {
    pub front: StencilFaceState,
    pub back: StencilFaceState,
    pub read_mask: u32,
    pub write_mask: u32,
}

impl From<StencilState> for wgpu::StencilState {
    fn from(value: StencilState) -> Self {
        Self {
            front: value.front.into(),
            back: value.back.into(),
            read_mask: value.read_mask,
            write_mask: value.write_mask,
        }
    }
}

/// Depth bias ("polygon offset") state — mirrors `wgpu::DepthBiasState`.
/// Defaults to disabled (all zero).
#[derive(Copy, Clone, PartialEq, Default)]
pub struct DepthBiasState {
    pub constant: i32,
    pub slope_scale: f32,
    pub clamp: f32,
}

impl From<DepthBiasState> for wgpu::DepthBiasState {
    fn from(value: DepthBiasState) -> Self {
        Self { constant: value.constant, slope_scale: value.slope_scale, clamp: value.clamp }
    }
}

/// Depth/stencil state of a render pipeline — mirrors `wgpu::DepthStencilState`.
#[derive(Clone, PartialEq)]
pub struct DepthStencilState {
    /// Format of the depth/stencil attachment. Must match the attachment
    /// bound at draw time.
    pub format: TextureFormat,
    /// Whether to write updated depth values. `None` if not depth-testing.
    pub depth_write_enabled: Option<bool>,
    /// Comparison function for the depth test. `None` if not depth-testing.
    pub depth_compare: Option<CompareFunction>,
    /// Stencil test state — [`StencilState::default()`] disables it.
    pub stencil: StencilState,
    /// Depth bias state — [`DepthBiasState::default()`] disables it.
    pub bias: DepthBiasState,
}

impl From<DepthStencilState> for wgpu::DepthStencilState {
    fn from(value: DepthStencilState) -> Self {
        Self {
            format: value.format.into(),
            depth_write_enabled: value.depth_write_enabled,
            depth_compare: value.depth_compare.map(Into::into),
            stencil: value.stencil.into(),
            bias: value.bias.into(),
        }
    }
}

/// Describes a render pipeline + its own bind group, the source type
/// [`GPUMaterial`] is built from via [`build_material`]. Fields are private —
/// the only way to construct one is [`MaterialBuilder`]:
/// `MaterialBuilder::new(shader_source).build()`.
pub struct Material {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    label: Option<&'static str>,
    /// WGSL source for both the vertex and fragment stage.
    shader_source: &'static str,
    /// Vertex stage entry point. Defaults to `"vs_main"`.
    vertex_entry: Option<&'static str>,
    /// Fragment stage entry point. Defaults to `"fs_main"`.
    fragment_entry: Option<&'static str>,
    /// Vertex buffer layouts, in the order buffers will be bound at draw
    /// time (e.g. [`Vertex::layout()`](super::mesh::Vertex::layout)).
    vertex_layouts: Vec<VertexBufferLayout>,
    /// This material's bind groups, in `@group(N)` order — set via
    /// [`entries`](MaterialBuilder::entries), whose docs cover the full shape.
    groups: Vec<super::layout::GroupEntry>,
    /// Face culling mode. Defaults to `Some(Face::Back)`.
    cull_mode: Option<Face>,
    /// Depth/stencil state. `None` disables depth testing.
    depth: Option<DepthStencilState>,
    /// Color target states — one per fragment shader output. See
    /// [`DEFAULT_TARGET`] for a ready-made single-target default.
    targets: Vec<ColorTargetState>,
    /// Rasterizer polygon mode. Defaults to `Fill`.
    polygon_mode: PolygonMode,
    /// Multisample count this pipeline renders at. Must match whatever
    /// render pass it's used in — `1` (no MSAA, the default) for an
    /// ordinary or offscreen target, or
    /// [`WGPUBackend::sample_count`](super::backend::WGPUBackend::sample_count)
    /// for a material meant to render into the (possibly MSAA) window
    /// surface via `ColorTarget::Default`. Passes mixing sample counts in
    /// one frame (an MSAA scene pass, a non-MSAA post-process/UI pass
    /// reading the resolved result) need each material to declare its own.
    sample_count: u32,
}

/// Builds a [`Material`]. Start from [`new`](Self::new) (or [`default`](Self::default)
/// for an opaque material with no shader source yet), chain the setters
/// below, then finish with [`build`](Self::build)/[`build_asset`](Self::build_asset).
pub struct MaterialBuilder {
    label: Option<&'static str>,
    shader_source: &'static str,
    vertex_entry: Option<&'static str>,
    fragment_entry: Option<&'static str>,
    vertex_layouts: Vec<VertexBufferLayout>,
    groups: Vec<super::layout::GroupEntry>,
    cull_mode: Option<Face>,
    depth: Option<DepthStencilState>,
    targets: Vec<ColorTargetState>,
    polygon_mode: PolygonMode,
    sample_count: u32,
}

impl Default for MaterialBuilder {
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

impl MaterialBuilder {
    /// Start building a material with the given WGSL shader source.
    /// All other fields are set to their defaults (see [`Default`]).
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

    /// Clear `vertex_entry` — let wgpu auto-detect the module's single
    /// `@vertex` function instead of naming one. Only valid if
    /// `shader_source` declares exactly one. The counterpart to
    /// [`with_vertex_entry`](Self::with_vertex_entry), which can only set it to `Some`.
    pub fn without_vertex_entry(mut self) -> Self {
        self.vertex_entry = None;
        self
    }

    pub fn with_fragment_entry(mut self, entry: &'static str) -> Self {
        self.fragment_entry = Some(entry);
        self
    }

    /// Clear `fragment_entry` — same rationale as
    /// [`without_vertex_entry`](Self::without_vertex_entry), for the `@fragment` stage.
    pub fn without_fragment_entry(mut self) -> Self {
        self.fragment_entry = None;
        self
    }

    pub fn with_vertex_layouts(mut self, layouts: Vec<VertexBufferLayout>) -> Self {
        self.vertex_layouts = layouts;
        self
    }

    /// This material's bind groups, in `@group(N)` order — position in `groups` *is* the
    /// `@group(N)` index a shader must declare to match: the first element is `@group(0)`,
    /// the second `@group(1)`, and so on. Each element is either:
    ///
    /// - [`GroupEntry::Own`](super::layout::GroupEntry::Own) — this material's own bind group
    ///   entries, built into a fresh layout internally. At most one of these is allowed — the
    ///   one group a [`GPUMaterialInstance`](super::instance::GPUMaterialInstance) binds
    ///   concrete resources against — `build_material` panics on a second one.
    /// - [`GroupEntry::Layout`](super::layout::GroupEntry::Layout) — an already-built layout
    ///   occupying this position directly: a camera, lights, or anything else external, e.g.
    ///   pulled from a [`GlobalLayoutPool`](super::layout::GlobalLayoutPool) via
    ///   [`GlobalLayoutPool::get`](super::layout::GlobalLayoutPool::get).
    ///
    /// `build_material` also panics if any `Own` entry is visible to the compute stage, or if
    /// `groups` needs more bind groups than the device's `max_bind_groups` allows (`wgpu`
    /// guarantees only 4) — list only the groups this material's shader actually declares.
    pub fn with_entries(mut self, groups: Vec<super::layout::GroupEntry>) -> Self {
        self.groups = groups;
        self
    }

    /// Face culling mode. Defaults to `Some(Face::Back)`.
    pub fn with_cull_mode(mut self, mode: Face) -> Self {
        self.cull_mode = Some(mode);
        self
    }

    /// Clear `cull_mode` — render both faces (no backface culling). The
    /// counterpart to [`with_cull_mode`](Self::with_cull_mode), which can only set it
    /// to `Some`.
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

    /// Logs a WARN for likely-forgotten configuration — not fatal, since
    /// there are legitimate reasons to leave these unset, but each is
    /// unusual enough to be worth flagging before it turns into a confusing
    /// wgpu validation error or a blank draw.
    fn validate(&self) {
        if self.targets.is_empty() {
            tracing::warn!(
                "MaterialBuilder{}: no color targets set — a render pipeline normally writes to \
                 at least one; consider calling .with_targets(...) (unless this is intentionally a \
                 depth-only pass)",
                self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
            );
        }
    }

    /// Consume the builder and return the finished [`Material`] value.
    pub fn build(self) -> Material {
        self.validate();
        Material {
            label: self.label,
            shader_source: self.shader_source,
            vertex_entry: self.vertex_entry,
            fragment_entry: self.fragment_entry,
            vertex_layouts: self.vertex_layouts,
            groups: self.groups,
            cull_mode: self.cull_mode,
            depth: self.depth,
            targets: self.targets,
            polygon_mode: self.polygon_mode,
            sample_count: self.sample_count,
        }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Material>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Material>) -> Handle<Material> {
        let material = self.build();
        assets.insert(name, material)
    }
}

/// Builds a render pipeline and its own bind group layout from `desc`.
///
/// Returns `None` — not a panic — if `desc.entries` contains a
/// [`GroupEntry::Global`](super::layout::GroupEntry::Global) not yet registered in `pool`; the
/// caller (`GPUMaterial::upload`) treats that exactly like any other unmet `Deps` and retries
/// next tick.
///
/// Panics if the one [`GroupEntry::Own`](super::layout::GroupEntry::Own) in `desc.entries`
/// (if any) is visible to the compute stage — [`BindingKind`](super::binding::BindingKind) is
/// shared with [`Compute`](super::compute::Compute), and this is the check that catches a
/// compute-only entry accidentally reused in a material instead of letting it fail deep inside
/// wgpu with a less specific error. The bind group layout itself comes from
/// [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder). The pipeline
/// layout is assembled directly from `desc.entries`, in order — position is the `@group(N)`
/// index — panicking if `desc.entries` contains more than one `GroupEntry::Own`, or needs more
/// bind groups than the device's `max_bind_groups` allows, turning either mistake into an
/// immediate, specific error instead of an opaque wgpu validation failure at draw time.
pub fn build_material(
    backend: &WGPUBackend,
    desc: &Material,
    pool: &super::layout::GlobalLayoutPool,
) -> Option<(RenderPipeline, BindGroupLayout)> {
    build_material_raw(&backend.device, desc, pool)
}

/// Panics if `desc.vertex_layouts`/`desc.targets` need more of a device's fixed-size pipeline
/// resources than it actually has — `max_vertex_buffers`, `max_vertex_attributes` (summed
/// across every vertex layout), `max_color_attachments` — turning what would otherwise be an
/// opaque wgpu validation panic deep inside `create_render_pipeline` into a clear message with
/// the actual count and the device's real limit.
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

/// Internal primitive behind [`build_material`] — used directly only by
/// tests, which have a raw `wgpu::Device` but no full [`WGPUBackend`].
pub(crate) fn build_material_raw(
    device: &wgpu::Device,
    desc: &Material,
    pool: &super::layout::GlobalLayoutPool,
) -> Option<(RenderPipeline, BindGroupLayout)> {
    check_material_limits(device, desc);

    let own_entries =
        super::layout::find_own_entries(desc.label, super::layout::PipelineKind::Material, &desc.groups);
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

/// A material uploaded to the GPU: a render pipeline plus the bind group
/// layout entries it expects, ready for a
/// [`GPUMaterialInstance`](super::instance::GPUMaterialInstance) to bind
/// actual resources against.
pub struct GPUMaterial {
    pub pipeline: RenderPipeline,
    layout: BindGroupLayout,
    entries: Vec<BindingEntry>,
}

impl super::binding::BindGroupTarget for GPUMaterial {
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

impl Asset<WGPUBackend> for Material {
    type Deps<'a> = crate::ecs::system::Res<'a, super::layout::GlobalLayoutPool>;

    fn upload<'a>(
        &self,
        backend: &WGPUBackend,
        pool: &crate::ecs::system::Res<'a, super::layout::GlobalLayoutPool>,
    ) -> Option<GPUMaterial> {
        let (pipeline, layout) = build_material(backend, self, pool)?;
        let entries =
            super::layout::find_own_entries(self.label, super::layout::PipelineKind::Material, &self.groups)
                .to_vec();

        Some(GPUMaterial { pipeline, layout, entries })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`Material`] asset pipeline. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MaterialPlugin, Material
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::{BindingEntry, BindingKind};
    use crate::wgpu::test_util::with_device;
    use crate::wgpu::vertex_format::{VertexAttribute, VertexFormat, VertexStepMode};

    const MINIMAL_SHADER: &str = r#"
        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        @fragment
        fn fs_main() -> @location(0) vec4<f32> {
            return vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    "#;

    #[test]
    fn a_compute_visible_own_entry_panics_before_touching_the_device() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![super::super::layout::GroupEntry::Own(vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE),
                }])])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE-visible material entry");
        });
    }

    #[test]
    fn no_entries_at_all_builds_without_panicking() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = MaterialBuilder::new(MINIMAL_SHADER).with_targets(DEFAULT_TARGET.to_vec()).build();
            build_material_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_layout_pulled_from_the_global_pool_ends_up_in_the_pipeline_layout() {
        with_device!(device, _queue, {
            let mut pool = super::super::layout::GlobalLayoutPool::new();
            pool.register("camera", crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device));

            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![super::super::layout::GroupEntry::Layout(pool.get("camera").unwrap())])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            build_material_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_global_entry_resolves_from_the_pool_at_build_time() {
        with_device!(device, _queue, {
            let mut pool = super::super::layout::GlobalLayoutPool::new();
            pool.register("camera", crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device));

            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![super::super::layout::GroupEntry::Global("camera")])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            build_material_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn a_global_entry_not_yet_registered_returns_none_instead_of_panicking() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new(); // "camera" never registered
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![super::super::layout::GroupEntry::Global("camera")])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            assert!(build_material_raw(&device, &desc, &pool).is_none());
        });
    }

    #[test]
    fn own_and_layout_groups_are_ordered_by_position() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let extra = crate::wgpu::binding::BindGroupLayoutBuilder::new().build_raw(&device);
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![
                    super::super::layout::GroupEntry::Own(vec![]),
                    super::super::layout::GroupEntry::Layout(extra),
                ])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            build_material_raw(&device, &desc, &pool).unwrap();
        });
    }

    #[test]
    fn more_than_one_own_group_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_entries(vec![
                    super::super::layout::GroupEntry::Own(vec![]),
                    super::super::layout::GroupEntry::Own(vec![]),
                ])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
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
            let desc =
                MaterialBuilder::new(MINIMAL_SHADER).with_entries(groups).with_targets(DEFAULT_TARGET.to_vec()).build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_bind_groups");
        });
    }

    #[test]
    fn exceeding_max_vertex_buffers_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let too_many = device.limits().max_vertex_buffers + 1;
            let layouts: Vec<VertexBufferLayout> = (0..too_many)
                .map(|_| VertexBufferLayout { array_stride: 4, step_mode: VertexStepMode::Vertex, attributes: vec![] })
                .collect();
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_vertex_layouts(layouts)
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_vertex_buffers");
        });
    }

    #[test]
    fn exceeding_max_vertex_attributes_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let too_many = device.limits().max_vertex_attributes + 1;
            let attributes: Vec<VertexAttribute> = (0..too_many)
                .map(|i| VertexAttribute { format: VertexFormat::Float32, offset: 0, shader_location: i })
                .collect();
            let desc = MaterialBuilder::new(MINIMAL_SHADER)
                .with_vertex_layouts(vec![VertexBufferLayout {
                    array_stride: 4,
                    step_mode: VertexStepMode::Vertex,
                    attributes,
                }])
                .with_targets(DEFAULT_TARGET.to_vec())
                .build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_vertex_attributes");
        });
    }

    #[test]
    fn exceeding_max_color_attachments_panics() {
        with_device!(device, _queue, {
            let pool = super::super::layout::GlobalLayoutPool::new();
            let too_many = device.limits().max_color_attachments + 1;
            let targets: Vec<ColorTargetState> = (0..too_many).map(|_| DEFAULT_TARGET[0].clone()).collect();
            let desc = MaterialBuilder::new(MINIMAL_SHADER).with_targets(targets).build();

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc, &pool);
            }));
            assert!(result.is_err(), "expected a panic for exceeding max_color_attachments");
        });
    }
}
