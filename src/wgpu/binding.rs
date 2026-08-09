//! Shared bind-group vocabulary for [`material`](super::material) and
//! [`compute`](super::compute) — a material's own bind group and a compute
//! pass's own bind group are described the same way, differing only in
//! which shader stage(s) can see each entry (a material entry can be
//! `FRAGMENT`/`VERTEX`/`VERTEX_FRAGMENT`; a compute entry is always exactly
//! `COMPUTE`). Every constructor below takes `visibility` explicitly rather
//! than guessing a default per module — [`build_material`](super::material::build_material)/
//! [`build_compute`](super::compute::build_compute) validate it's
//! appropriate for the pipeline kind they're building, panicking with a
//! clear message otherwise.
//!
//! Also useful directly (not just via `Material`/`Compute`)
//! any time you're building a bind group layout by hand — [`BindGroupLayoutBuilder`]
//! catches a duplicate `@binding(N)` with a clear panic instead of a wgpu
//! validation failure at draw time. Re-exported, along with [`buffers`](super::buffers),
//! from [`wgpu::prelude`](super::prelude).

use crate::wgpu::backend::WGPUBackend;
use crate::wgpu::flags::ShaderStages;
use crate::wgpu::texture_format::TextureFormat;

/// Specific type of a sample in a texture binding — mirrors
/// `wgpu::TextureSampleType`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum TextureSampleType {
    Float { filterable: bool },
    Depth,
    Sint,
    Uint,
}

impl From<TextureSampleType> for wgpu::TextureSampleType {
    fn from(value: TextureSampleType) -> Self {
        match value {
            TextureSampleType::Float { filterable } => Self::Float { filterable },
            TextureSampleType::Depth => Self::Depth,
            TextureSampleType::Sint => Self::Sint,
            TextureSampleType::Uint => Self::Uint,
        }
    }
}

/// Dimensions of a texture view — mirrors `wgpu::TextureViewDimension`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}

impl From<TextureViewDimension> for wgpu::TextureViewDimension {
    fn from(value: TextureViewDimension) -> Self {
        match value {
            TextureViewDimension::D1 => Self::D1,
            TextureViewDimension::D2 => Self::D2,
            TextureViewDimension::D2Array => Self::D2Array,
            TextureViewDimension::Cube => Self::Cube,
            TextureViewDimension::CubeArray => Self::CubeArray,
            TextureViewDimension::D3 => Self::D3,
        }
    }
}

/// Access mode for a storage texture binding — mirrors
/// `wgpu::StorageTextureAccess`.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum StorageTextureAccess {
    WriteOnly,
    ReadOnly,
    ReadWrite,
    Atomic,
}

impl From<StorageTextureAccess> for wgpu::StorageTextureAccess {
    fn from(value: StorageTextureAccess) -> Self {
        match value {
            StorageTextureAccess::WriteOnly => Self::WriteOnly,
            StorageTextureAccess::ReadOnly => Self::ReadOnly,
            StorageTextureAccess::ReadWrite => Self::ReadWrite,
            StorageTextureAccess::Atomic => Self::Atomic,
        }
    }
}

/// What kind of resource a single [`BindingEntry`] binds, the wgpu binding
/// parameters that go with it, and which shader stage(s) can see it.
/// Construct via the `texture_*`/`*_buffer`/`sampler`/`storage_texture`
/// associated functions rather than the variants directly — they fill in
/// the usual defaults (filterable float textures, non-dynamic buffers) so
/// only the cases that actually differ need spelling out.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BindingKind {
    /// A sampled texture (`texture_2d<f32>` and friends in WGSL).
    Texture {
        visibility: ShaderStages,
        sample_type: TextureSampleType,
        view_dimension: TextureViewDimension,
        multisampled: bool,
    },
    /// A texture bound for direct read/write access (`textureStore`/
    /// `textureLoad` in WGSL) rather than sampling.
    StorageTexture {
        visibility: ShaderStages,
        format: TextureFormat,
        access: StorageTextureAccess,
        view_dimension: TextureViewDimension,
    },
    /// A filtering sampler.
    Sampler { visibility: ShaderStages },
    /// A comparison sampler (e.g. for shadow-map `textureSampleCompare`).
    ComparisonSampler { visibility: ShaderStages },
    /// A uniform buffer.
    UniformBuffer {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
    /// A read-only storage buffer.
    StorageBufferReadOnly {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
    /// A read-write storage buffer.
    StorageBufferReadWrite {
        visibility: ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<u64>,
    },
}

impl BindingKind {
    /// A filterable, non-multisampled 2D texture — the common case.
    pub fn texture_2d(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        }
    }

    /// A filterable, non-multisampled 2D texture array.
    pub fn texture_2d_array(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2Array,
            multisampled: false,
        }
    }

    /// A filterable, non-multisampled cubemap texture.
    pub fn texture_cubemap(visibility: ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::Cube,
            multisampled: false,
        }
    }

    /// A storage texture bound for direct read/write access in a shader.
    pub fn storage_texture(
        visibility: ShaderStages,
        format: TextureFormat,
        access: StorageTextureAccess,
        view_dimension: TextureViewDimension,
    ) -> Self {
        Self::StorageTexture { visibility, format, access, view_dimension }
    }

    /// A filtering sampler.
    pub fn sampler(visibility: ShaderStages) -> Self {
        Self::Sampler { visibility }
    }

    /// A comparison sampler (e.g. for shadow-map `textureSampleCompare`).
    pub fn comparison_sampler(visibility: ShaderStages) -> Self {
        Self::ComparisonSampler { visibility }
    }

    /// A uniform buffer, bound as a whole (no dynamic offset).
    pub fn uniform_buffer(visibility: ShaderStages) -> Self {
        Self::UniformBuffer { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A uniform buffer bound with a dynamic offset — `element_size` is the byte size of one
    /// element (before alignment padding). Use
    /// [`DynamicBufferBuilder`](crate::wgpu::buffers::DynamicBufferBuilder)
    /// to allocate the backing buffer and
    /// [`BindGroupBuilder::dynamic_buffer`](crate::wgpu::buffers::BindGroupBuilder::dynamic_buffer)
    /// (not `.buffer()`/`buffer.as_entire_binding()`) to bind it — the entry
    /// must be scoped to one element's size, not the whole buffer, or
    /// dynamic offsets will fail validation.
    pub fn dynamic_uniform_buffer(visibility: ShaderStages, element_size: u64) -> Self {
        Self::UniformBuffer {
            visibility,
            has_dynamic_offset: true,
            min_binding_size: Some(element_size),
        }
    }

    /// A read-only storage buffer, bound as a whole (no dynamic offset).
    pub fn storage_buffer_read_only(visibility: ShaderStages) -> Self {
        Self::StorageBufferReadOnly { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A read-write storage buffer, bound as a whole (no dynamic offset).
    pub fn storage_buffer_read_write(visibility: ShaderStages) -> Self {
        Self::StorageBufferReadWrite { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A storage buffer bound with a dynamic offset. See
    /// [`Self::dynamic_uniform_buffer`].
    pub fn dynamic_storage_buffer(visibility: ShaderStages, element_size: u64, read_only: bool) -> Self {
        let has_dynamic_offset = true;
        let min_binding_size = Some(element_size);
        if read_only {
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size }
        } else {
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size }
        }
    }

    /// Which shader stage(s) this binding is visible to.
    pub fn visibility(&self) -> ShaderStages {
        match self {
            Self::Texture { visibility, .. }
            | Self::StorageTexture { visibility, .. }
            | Self::Sampler { visibility }
            | Self::ComparisonSampler { visibility }
            | Self::UniformBuffer { visibility, .. }
            | Self::StorageBufferReadOnly { visibility, .. }
            | Self::StorageBufferReadWrite { visibility, .. } => *visibility,
        }
    }

    pub(crate) fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            Self::Texture { visibility, sample_type, view_dimension, multisampled } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Texture {
                    sample_type: (*sample_type).into(),
                    view_dimension: (*view_dimension).into(),
                    multisampled: *multisampled,
                },
                count: None,
            },
            Self::StorageTexture { visibility, format, access, view_dimension } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::StorageTexture {
                    access: (*access).into(),
                    format: (*format).into(),
                    view_dimension: (*view_dimension).into(),
                },
                count: None,
            },
            Self::Sampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            Self::ComparisonSampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            Self::UniformBuffer { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: (*visibility).into(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
                },
                count: None,
            },
        }
    }
}

/// A `wgpu::BindGroupLayout`, opaque — built only via
/// [`BindGroupLayoutBuilder::build`]. There's no way to reach the underlying
/// `wgpu::BindGroupLayout` from outside this crate. `Clone` because a layout is often wired
/// into more than one material/compute pass — e.g. via
/// [`GroupEntry::Layout`](super::layout::GroupEntry::Layout), or a
/// [`GlobalLayoutPool`](super::layout::GlobalLayoutPool) registration handed out by
/// [`GlobalLayoutPool::get`](super::layout::GlobalLayoutPool::get) — cheap, the same
/// `Arc`-backed handle underneath.
#[derive(Clone)]
pub struct BindGroupLayout(wgpu::BindGroupLayout);

impl BindGroupLayout {
    pub(crate) fn raw(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

/// One binding within a material's or compute pass's own bind group (see
/// `Material::entries`/`Compute::entries`).
#[derive(Clone)]
pub struct BindingEntry {
    /// Shader-facing name, used only in panic/diagnostic messages — has no
    /// effect on the actual binding.
    pub name: &'static str,
    /// The `@binding(N)` this entry occupies within its bind group. Explicit rather than
    /// inferred from position in `entries`, so it matches the shader unambiguously.
    pub binding: u32,
    /// What resource this binding expects, its wgpu binding parameters,
    /// and which shader stage(s) can see it.
    pub kind: BindingKind,
}

/// Builds a `wgpu::BindGroupLayout` one [`BindingEntry`] at a time.
///
/// ```ignore
/// let layout = BindGroupLayoutBuilder::new()
///     .with_label("camera_layout")
///     .with_entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
///     .build(&backend);
/// ```
///
/// [`build`](Self::build) panics if two entries claim the same `@binding(N)`
/// — this makes a shader-mismatched binding layout fail loudly here instead
/// of silently misbehaving at draw/dispatch time.
#[derive(Default)]
pub struct BindGroupLayoutBuilder<'a> {
    label: Option<&'a str>,
    entries: Vec<BindingEntry>,
}

impl<'a> BindGroupLayoutBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Appends one entry. Call repeatedly for a multi-entry layout.
    pub fn with_entry(mut self, name: &'static str, binding: u32, kind: BindingKind) -> Self {
        self.entries.push(BindingEntry { name, binding, kind });
        self
    }

    /// Appends every entry from `entries` — for building from an
    /// already-collected `Vec<BindingEntry>` (e.g.
    /// `Material::entries`) rather than one at a time.
    pub fn with_entries(mut self, entries: impl IntoIterator<Item = BindingEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> BindGroupLayout {
        self.build_raw(&backend.device)
    }

    /// Internal primitive behind [`build`](Self::build) — used directly only
    /// by tests, which have a raw `wgpu::Device` but no full [`WGPUBackend`].
    pub(crate) fn build_raw(self, device: &wgpu::Device) -> BindGroupLayout {
        let layout_entries: Vec<_> =
            self.entries.iter().map(|e| e.kind.layout_entry(e.binding)).collect();

        let mut seen = std::collections::HashSet::new();
        for e in &self.entries {
            if !seen.insert(e.binding) {
                panic!(
                    "binding {} assigned more than once building bind group layout{} (entry '{}')",
                    e.binding,
                    self.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                    e.name
                );
            }
        }

        BindGroupLayout(device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: self.label,
            entries: &layout_entries,
        }))
    }
}

/// Implemented by [`GPUMaterial`](super::material::GPUMaterial) and
/// [`GPUCompute`](super::compute::GPUCompute) — anything with its own bind
/// group layout and named entries that a
/// [`GPUBindingInstance`](super::instance::GPUBindingInstance) can bind
/// concrete resources against.
pub trait BindGroupTarget {
    fn bind_group_layout(&self) -> &BindGroupLayout;
    fn binding_entries(&self) -> &[BindingEntry];
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure logic — no device needed.

    #[test]
    fn visibility_reports_back_exactly_what_each_constructor_was_given() {
        let stages = ShaderStages::VERTEX_FRAGMENT;
        assert!(BindingKind::texture_2d(stages).visibility() == stages);
        assert!(BindingKind::sampler(stages).visibility() == stages);
        assert!(BindingKind::uniform_buffer(stages).visibility() == stages);
        assert!(
            BindingKind::storage_buffer_read_only(ShaderStages::COMPUTE).visibility()
                == ShaderStages::COMPUTE
        );
        assert!(
            BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE).visibility()
                == ShaderStages::COMPUTE
        );
    }

    #[test]
    fn dynamic_storage_buffer_picks_read_only_or_read_write_by_flag() {
        let read_only = BindingKind::dynamic_storage_buffer(ShaderStages::COMPUTE, 16, true);
        let read_write = BindingKind::dynamic_storage_buffer(ShaderStages::COMPUTE, 16, false);
        assert!(matches!(read_only, BindingKind::StorageBufferReadOnly { .. }));
        assert!(matches!(read_write, BindingKind::StorageBufferReadWrite { .. }));
    }

    // Device-dependent — see `test_util` for why these skip instead of
    // failing when no adapter is available.

    #[test]
    fn unique_bindings_build_without_panicking() {
        crate::wgpu::test_util::with_device!(device, _queue, {
            BindGroupLayoutBuilder::new()
                .with_entry("a", 0, BindingKind::texture_2d(ShaderStages::FRAGMENT))
                .with_entry("b", 1, BindingKind::sampler(ShaderStages::FRAGMENT))
                .build_raw(&device);
        });
    }

    #[test]
    fn two_entries_claiming_the_same_binding_panics() {
        crate::wgpu::test_util::with_device!(device, _queue, {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                BindGroupLayoutBuilder::new()
                    .with_entry("a", 0, BindingKind::texture_2d(ShaderStages::FRAGMENT))
                    .with_entry("b", 0, BindingKind::sampler(ShaderStages::FRAGMENT))
                    .build_raw(&device);
            }));
            assert!(result.is_err(), "expected a panic for a duplicate @binding(0)");
        });
    }
}
