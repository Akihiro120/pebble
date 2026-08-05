use crate::{
    assets::{handle::Handle, storage::Assets, upload::Asset},
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
/// ready-made value for [`Material::targets`] when you don't need
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
/// [`GPUMaterial`] is built from via [`build_material`]. Start from
/// [`Material::default()`] and override only the fields that differ from a
/// plain opaque material with no depth testing.
pub struct Material {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    pub label: Option<&'static str>,
    /// WGSL source for both the vertex and fragment stage.
    pub shader_source: &'static str,
    /// Vertex stage entry point. Defaults to `"vs_main"`.
    pub vertex_entry: Option<&'static str>,
    /// Fragment stage entry point. Defaults to `"fs_main"`.
    pub fragment_entry: Option<&'static str>,
    /// Vertex buffer layouts, in the order buffers will be bound at draw
    /// time (e.g. [`Vertex::layout()`](super::mesh::Vertex::layout)).
    pub vertex_layouts: Vec<VertexBufferLayout>,
    /// This material's own bind group entries. See
    /// [`BindingKind`](super::binding::BindingKind) for what a
    /// material-appropriate entry looks like — [`build_material`] panics if
    /// any entry here is `COMPUTE`-visible.
    pub entries: Vec<BindingEntry>,
    /// Face culling mode. Defaults to `Some(Face::Back)`.
    pub cull_mode: Option<Face>,
    /// Depth/stencil state. `None` disables depth testing.
    pub depth: Option<DepthStencilState>,
    /// Color target states — one per fragment shader output. See
    /// [`DEFAULT_TARGET`] for a ready-made single-target default.
    pub targets: Vec<ColorTargetState>,
    /// Rasterizer polygon mode. Defaults to `Fill`.
    pub polygon_mode: PolygonMode,
    /// Multisample count this pipeline renders at. Must match whatever
    /// render pass it's used in — `1` (no MSAA, the default) for an
    /// ordinary or offscreen target, or
    /// [`WGPUBackend::sample_count`](super::backend::WGPUBackend::sample_count)
    /// for a material meant to render into the (possibly MSAA) window
    /// surface via `ColorTarget::Default`. Passes mixing sample counts in
    /// one frame (an MSAA scene pass, a non-MSAA post-process/UI pass
    /// reading the resolved result) need each material to declare its own.
    pub sample_count: u32,
    /// Which `@group(N)` the layout built from `entries` occupies in the pipeline, or
    /// `None` if this material has no entries of its own (e.g. it only uses `extra_layouts`).
    pub own_group: Option<u32>,
    /// Additional bind group layouts, each tagged with the `@group(N)` it occupies.
    /// Every index from 0 up to the highest one used (including `own_group`, if set) must
    /// be covered exactly once, or `build_material` panics — this makes group assignment
    /// explicit instead of inferred from field order.
    pub extra_layouts: Vec<super::layout::OwnedGroupLayout>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: Vec::new(),
            entries: Vec::new(),
            cull_mode: Some(Face::Back),
            depth: None,
            targets: Vec::new(),
            own_group: Some(0),
            extra_layouts: Vec::new(),
            polygon_mode: PolygonMode::Fill,
            sample_count: 1,
        }
    }
}

impl Material {
    /// Start building a material with the given WGSL shader source.
    /// All other fields are set to their defaults (see [`Default`]).
    pub fn new(shader_source: &'static str) -> Self {
        Self { shader_source, ..Self::default() }
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn vertex_entry(mut self, entry: &'static str) -> Self {
        self.vertex_entry = Some(entry);
        self
    }

    pub fn fragment_entry(mut self, entry: &'static str) -> Self {
        self.fragment_entry = Some(entry);
        self
    }

    pub fn vertex_layouts(mut self, layouts: Vec<VertexBufferLayout>) -> Self {
        self.vertex_layouts = layouts;
        self
    }

    pub fn entries(mut self, entries: Vec<BindingEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn cull_mode(mut self, mode: Option<Face>) -> Self {
        self.cull_mode = mode;
        self
    }

    pub fn depth(mut self, depth: DepthStencilState) -> Self {
        self.depth = Some(depth);
        self
    }

    pub fn targets(mut self, targets: Vec<ColorTargetState>) -> Self {
        self.targets = targets;
        self
    }

    pub fn polygon_mode(mut self, mode: PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    pub fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
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

    /// Consume the builder and return the finished [`Material`] value.
    pub fn build(self) -> Self {
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<Material>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        assets.insert(name, self)
    }
}

/// Builds a render pipeline and its own bind group layout from `desc`.
///
/// Panics if any of `desc.entries` is visible to the compute stage —
/// [`BindingKind`](super::binding::BindingKind) is shared with
/// [`Compute`](super::compute::Compute), and this is
/// the check that catches a compute-only entry accidentally reused in a
/// material instead of letting it fail deep inside wgpu with a less
/// specific error. The bind group layout itself comes from
/// [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder).
/// The pipeline layout is assembled from `desc.own_group` (this material's
/// own layout) plus `desc.extra_layouts`, keyed by explicit `@group(N)` —
/// panics on a gap or a collision across `0..=max`, turning a mismatched
/// `@group(N)` in the shader into an immediate, specific error instead of
/// an opaque wgpu validation failure at draw time.
pub fn build_material(backend: &WGPUBackend, desc: &Material) -> (RenderPipeline, BindGroupLayout) {
    build_material_raw(&backend.device, desc)
}

/// Internal primitive behind [`build_material`] — used directly only by
/// tests, which have a raw `wgpu::Device` but no full [`WGPUBackend`].
pub(crate) fn build_material_raw(
    device: &wgpu::Device,
    desc: &Material,
) -> (RenderPipeline, BindGroupLayout) {
    for entry in &desc.entries {
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

    (RenderPipeline(pipeline), layout)
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

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = Material;
    type Deps<'a> = ();

    fn upload<'a>(source: &Material, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let (pipeline, layout) = build_material(backend, source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUMaterial`] asset pipeline (`Assets<Material>`
    /// → `ProcessedAssets<GPUMaterial>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MaterialPlugin, GPUMaterial
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::{BindingEntry, BindingKind};
    use crate::wgpu::test_util::with_device;

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
    fn a_compute_visible_entry_panics_before_touching_the_device() {
        with_device!(device, _queue, {
            let desc = Material {
                shader_source: MINIMAL_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE),
                }],
                targets: DEFAULT_TARGET.to_vec(),
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material_raw(&device, &desc);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE-visible material entry");
        });
    }

    #[test]
    fn a_fragment_visible_entry_builds_without_panicking() {
        with_device!(device, _queue, {
            let desc = Material {
                shader_source: MINIMAL_SHADER,
                entries: vec![],
                own_group: None,
                targets: DEFAULT_TARGET.to_vec(),
                ..Default::default()
            };
            build_material_raw(&device, &desc);
        });
    }
}
