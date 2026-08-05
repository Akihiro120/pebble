//! Mirrors `wgpu::VertexFormat`/`VertexStepMode`/`VertexAttribute` so a
//! custom vertex struct's layout (beyond the built-in
//! [`Vertex`](super::mesh::Vertex)/[`InstanceVertex`](super::mesh::InstanceVertex))
//! can be built with zero raw wgpu. Unlike `wgpu::VertexBufferLayout` (which
//! borrows a `'static` attribute slice), [`VertexBufferLayout`] owns its
//! attributes — simpler to build, converted to wgpu's borrowed shape
//! internally by [`build_material`](super::material::build_material) in a
//! scope that doesn't need `'static`.

/// Format of a single vertex attribute — mirrors `wgpu::VertexFormat`
/// exactly (same variants, same names).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    Uint8,
    Uint8x2,
    Uint8x4,
    Sint8,
    Sint8x2,
    Sint8x4,
    Unorm8,
    Unorm8x2,
    Unorm8x4,
    Snorm8,
    Snorm8x2,
    Snorm8x4,
    Uint16,
    Uint16x2,
    Uint16x4,
    Sint16,
    Sint16x2,
    Sint16x4,
    Unorm16,
    Unorm16x2,
    Unorm16x4,
    Snorm16,
    Snorm16x2,
    Snorm16x4,
    Float16,
    Float16x2,
    Float16x4,
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
    Float64,
    Float64x2,
    Float64x3,
    Float64x4,
    Unorm10_10_10_2,
    Unorm8x4Bgra,
}

impl From<VertexFormat> for wgpu::VertexFormat {
    fn from(format: VertexFormat) -> Self {
        match format {
            VertexFormat::Uint8 => Self::Uint8,
            VertexFormat::Uint8x2 => Self::Uint8x2,
            VertexFormat::Uint8x4 => Self::Uint8x4,
            VertexFormat::Sint8 => Self::Sint8,
            VertexFormat::Sint8x2 => Self::Sint8x2,
            VertexFormat::Sint8x4 => Self::Sint8x4,
            VertexFormat::Unorm8 => Self::Unorm8,
            VertexFormat::Unorm8x2 => Self::Unorm8x2,
            VertexFormat::Unorm8x4 => Self::Unorm8x4,
            VertexFormat::Snorm8 => Self::Snorm8,
            VertexFormat::Snorm8x2 => Self::Snorm8x2,
            VertexFormat::Snorm8x4 => Self::Snorm8x4,
            VertexFormat::Uint16 => Self::Uint16,
            VertexFormat::Uint16x2 => Self::Uint16x2,
            VertexFormat::Uint16x4 => Self::Uint16x4,
            VertexFormat::Sint16 => Self::Sint16,
            VertexFormat::Sint16x2 => Self::Sint16x2,
            VertexFormat::Sint16x4 => Self::Sint16x4,
            VertexFormat::Unorm16 => Self::Unorm16,
            VertexFormat::Unorm16x2 => Self::Unorm16x2,
            VertexFormat::Unorm16x4 => Self::Unorm16x4,
            VertexFormat::Snorm16 => Self::Snorm16,
            VertexFormat::Snorm16x2 => Self::Snorm16x2,
            VertexFormat::Snorm16x4 => Self::Snorm16x4,
            VertexFormat::Float16 => Self::Float16,
            VertexFormat::Float16x2 => Self::Float16x2,
            VertexFormat::Float16x4 => Self::Float16x4,
            VertexFormat::Float32 => Self::Float32,
            VertexFormat::Float32x2 => Self::Float32x2,
            VertexFormat::Float32x3 => Self::Float32x3,
            VertexFormat::Float32x4 => Self::Float32x4,
            VertexFormat::Uint32 => Self::Uint32,
            VertexFormat::Uint32x2 => Self::Uint32x2,
            VertexFormat::Uint32x3 => Self::Uint32x3,
            VertexFormat::Uint32x4 => Self::Uint32x4,
            VertexFormat::Sint32 => Self::Sint32,
            VertexFormat::Sint32x2 => Self::Sint32x2,
            VertexFormat::Sint32x3 => Self::Sint32x3,
            VertexFormat::Sint32x4 => Self::Sint32x4,
            VertexFormat::Float64 => Self::Float64,
            VertexFormat::Float64x2 => Self::Float64x2,
            VertexFormat::Float64x3 => Self::Float64x3,
            VertexFormat::Float64x4 => Self::Float64x4,
            VertexFormat::Unorm10_10_10_2 => Self::Unorm10_10_10_2,
            VertexFormat::Unorm8x4Bgra => Self::Unorm8x4Bgra,
        }
    }
}

/// Whether a vertex buffer is indexed by vertex or by instance — mirrors
/// `wgpu::VertexStepMode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexStepMode {
    Vertex,
    Instance,
}

impl From<VertexStepMode> for wgpu::VertexStepMode {
    fn from(mode: VertexStepMode) -> Self {
        match mode {
            VertexStepMode::Vertex => Self::Vertex,
            VertexStepMode::Instance => Self::Instance,
        }
    }
}

/// One vertex attribute within a [`VertexBufferLayout`] — mirrors
/// `wgpu::VertexAttribute`.
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

/// Layout of one vertex buffer's attributes — owns its attribute list
/// (unlike `wgpu::VertexBufferLayout`, which borrows a `'static` slice), so
/// it can be built and passed around without lifetime ceremony.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexBufferLayout {
    pub array_stride: u64,
    pub step_mode: VertexStepMode,
    pub attributes: Vec<VertexAttribute>,
}
