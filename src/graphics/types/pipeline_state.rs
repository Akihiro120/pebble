use crate::graphics::types::{
    BlendFactor, BlendOperation, CompareFunction, StencilOperation, TextureFormat, VertexFormat, VertexStepMode,
    flags::ColorWrites,
};

/// One field within a vertex — its format, byte offset, and `@location` in
/// the shader.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VertexAttribute {
    pub format: VertexFormat,
    pub offset: u64,
    pub shader_location: u32,
}

impl From<VertexAttribute> for wgpu::VertexAttribute {
    fn from(attr: VertexAttribute) -> Self {
        Self {
            format: attr.format.into(),
            offset: attr.offset,
            shader_location: attr.shader_location,
        }
    }
}

/// One vertex buffer's layout — stride, step mode, and its [`VertexAttribute`]s.
/// Usually built by a vertex type's own `layout()` method (e.g. [`Vertex::layout`](crate::graphics::pipeline::mesh::Vertex::layout))
/// rather than by hand.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexBufferLayout {
    pub array_stride: u64,
    pub step_mode: VertexStepMode,
    pub attributes: Vec<VertexAttribute>,
}

/// How one channel (color or alpha) blends — src/dst factors plus the
/// combining operation. [`REPLACE`](Self::REPLACE)/[`OVER`](Self::OVER) cover the common cases.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlendComponent {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

impl BlendComponent {
    pub const REPLACE: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    };

    pub const OVER: Self = Self {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    };
}

impl Default for BlendComponent {
    /// [`REPLACE`](Self::REPLACE) — no blending.
    fn default() -> Self {
        Self::REPLACE
    }
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

/// Color + alpha blending for one [`ColorTargetState`]. Presets:
/// [`REPLACE`](Self::REPLACE) (no blending), [`ALPHA_BLENDING`](Self::ALPHA_BLENDING),
/// [`PREMULTIPLIED_ALPHA_BLENDING`](Self::PREMULTIPLIED_ALPHA_BLENDING).
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

impl BlendState {
    pub const REPLACE: Self = Self { color: BlendComponent::REPLACE, alpha: BlendComponent::REPLACE };

    pub const ALPHA_BLENDING: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent::OVER,
    };

    pub const PREMULTIPLIED_ALPHA_BLENDING: Self =
        Self { color: BlendComponent::OVER, alpha: BlendComponent::OVER };
}

impl Default for BlendState {
    /// [`REPLACE`](Self::REPLACE) — no blending.
    fn default() -> Self {
        Self::REPLACE
    }
}

impl From<BlendState> for wgpu::BlendState {
    fn from(value: BlendState) -> Self {
        Self { color: value.color.into(), alpha: value.alpha.into() }
    }
}

/// One color attachment's output format, blend mode, and write mask —
/// passed to [`Material::with_targets`](crate::graphics::pipeline::material::Material::with_targets).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ColorTargetState {
    pub format: TextureFormat,
    pub blend: Option<BlendState>,
    pub write_mask: ColorWrites,
}

impl Default for ColorTargetState {
    /// Opaque `Rgba8Unorm`, no blending, writes all channels — kept in sync
    /// with [`DEFAULT_TARGET`], which just wraps this in a single-element `Vec`
    /// for [`Material::with_targets`](crate::graphics::pipeline::material::Material::with_targets).
    fn default() -> Self {
        Self { format: TextureFormat::Rgba8Unorm, blend: None, write_mask: ColorWrites::ALL }
    }
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

/// A single opaque `Rgba8Unorm` target with no blending — a reasonable
/// default for [`Material::with_targets`](crate::graphics::pipeline::material::Material::with_targets).
pub const DEFAULT_TARGET: [ColorTargetState; 1] = [ColorTargetState {
    format: TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: ColorWrites::ALL,
}];

/// Stencil test/write behavior for one face — see [`StencilState`]. Defaults
/// to [`IGNORE`](Self::IGNORE) (stencil test always passes, no writes).
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

/// Front/back stencil face state plus read/write masks — part of [`DepthStencilState`].
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

/// Depth bias ("polygon offset") — nudges depth values to avoid z-fighting,
/// e.g. for shadow-acne mitigation. Part of [`DepthStencilState`].
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

/// Depth/stencil buffer state for a pipeline — passed to
/// [`Material::with_depth`](crate::graphics::pipeline::material::Material::with_depth).
#[derive(Clone, PartialEq)]
pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth_write_enabled: Option<bool>,
    pub depth_compare: Option<CompareFunction>,
    pub stencil: StencilState,
    pub bias: DepthBiasState,
}

impl DepthStencilState {
    /// A standard depth-tested/depth-written `Depth32Float` buffer with a
    /// `Less` compare and no stencil — the common case for opaque 3D
    /// geometry. A reasonable default for [`Material::with_depth`](crate::graphics::pipeline::material::Material::with_depth);
    /// override `format` if your depth attachment uses a different one
    /// (e.g. `Depth32FloatStencil8`).
    pub const DEFAULT: Self = Self {
        format: TextureFormat::Depth32Float,
        depth_write_enabled: Some(true),
        depth_compare: Some(CompareFunction::Less),
        stencil: StencilState {
            front: StencilFaceState::IGNORE,
            back: StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: DepthBiasState { constant: 0, slope_scale: 0.0, clamp: 0.0 },
    };
}

impl Default for DepthStencilState {
    /// [`DEFAULT`](Self::DEFAULT).
    fn default() -> Self {
        Self::DEFAULT
    }
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
