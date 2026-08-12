use crate::graphics::types::{
    BlendFactor, BlendOperation, CompareFunction, StencilOperation, TextureFormat, VertexFormat, VertexStepMode,
    flags::ColorWrites,
};

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

#[derive(Clone, Debug, PartialEq)]
pub struct VertexBufferLayout {
    pub array_stride: u64,
    pub step_mode: VertexStepMode,
    pub attributes: Vec<VertexAttribute>,
}

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

impl From<BlendComponent> for wgpu::BlendComponent {
    fn from(value: BlendComponent) -> Self {
        Self {
            src_factor: value.src_factor.into(),
            dst_factor: value.dst_factor.into(),
            operation: value.operation.into(),
        }
    }
}

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

impl From<BlendState> for wgpu::BlendState {
    fn from(value: BlendState) -> Self {
        Self { color: value.color.into(), alpha: value.alpha.into() }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ColorTargetState {
    pub format: TextureFormat,
    pub blend: Option<BlendState>,
    pub write_mask: ColorWrites,
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

pub const DEFAULT_TARGET: [ColorTargetState; 1] = [ColorTargetState {
    format: TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: ColorWrites::ALL,
}];

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

#[derive(Clone, PartialEq)]
pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth_write_enabled: Option<bool>,
    pub depth_compare: Option<CompareFunction>,
    pub stencil: StencilState,
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
