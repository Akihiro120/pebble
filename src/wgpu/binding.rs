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
//! Also useful directly (not just via `MaterialDescriptor`/`ComputeDescriptor`)
//! any time you're building a bind group layout by hand — [`BindGroupLayoutBuilder`]
//! catches a duplicate `@binding(N)` with a clear panic instead of a wgpu
//! validation failure at draw time. Re-exported, along with [`buffers`](super::buffers),
//! from [`wgpu::prelude`](super::prelude).

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
        visibility: wgpu::ShaderStages,
        sample_type: wgpu::TextureSampleType,
        view_dimension: wgpu::TextureViewDimension,
        multisampled: bool,
    },
    /// A texture bound for direct read/write access (`textureStore`/
    /// `textureLoad` in WGSL) rather than sampling.
    StorageTexture {
        visibility: wgpu::ShaderStages,
        format: wgpu::TextureFormat,
        access: wgpu::StorageTextureAccess,
        view_dimension: wgpu::TextureViewDimension,
    },
    /// A filtering sampler.
    Sampler { visibility: wgpu::ShaderStages },
    /// A comparison sampler (e.g. for shadow-map `textureSampleCompare`).
    ComparisonSampler { visibility: wgpu::ShaderStages },
    /// A uniform buffer.
    UniformBuffer {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    /// A read-only storage buffer.
    StorageBufferReadOnly {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
    /// A read-write storage buffer.
    StorageBufferReadWrite {
        visibility: wgpu::ShaderStages,
        has_dynamic_offset: bool,
        min_binding_size: Option<wgpu::BufferSize>,
    },
}

impl BindingKind {
    /// A filterable, non-multisampled 2D texture — the common case.
    pub fn texture_2d(visibility: wgpu::ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    }

    /// Same as [`texture_2d`](Self::texture_2d) but for a 2D texture array
    /// (see [`GPUTextureArray`](super::texture_array::GPUTextureArray)).
    pub fn texture_2d_array(visibility: wgpu::ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        }
    }

    /// Same as [`texture_2d`](Self::texture_2d) but for a cubemap (see
    /// [`GPUCubemap`](super::cubemap::GPUCubemap)).
    pub fn texture_cubemap(visibility: wgpu::ShaderStages) -> Self {
        Self::Texture {
            visibility,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::Cube,
            multisampled: false,
        }
    }

    /// A storage texture bound for direct read/write/read-write access
    /// (per `access`) rather than sampling.
    pub fn storage_texture(
        visibility: wgpu::ShaderStages,
        format: wgpu::TextureFormat,
        access: wgpu::StorageTextureAccess,
        view_dimension: wgpu::TextureViewDimension,
    ) -> Self {
        Self::StorageTexture { visibility, format, access, view_dimension }
    }

    /// A filtering sampler.
    pub fn sampler(visibility: wgpu::ShaderStages) -> Self {
        Self::Sampler { visibility }
    }

    /// A comparison sampler (e.g. for shadow-map `textureSampleCompare`).
    pub fn comparison_sampler(visibility: wgpu::ShaderStages) -> Self {
        Self::ComparisonSampler { visibility }
    }

    /// A uniform buffer, bound as a whole (no dynamic offset) — the common
    /// case. See [`dynamic_uniform_buffer`](Self::dynamic_uniform_buffer)
    /// for the per-draw/per-dispatch-offset variant.
    pub fn uniform_buffer(visibility: wgpu::ShaderStages) -> Self {
        Self::UniformBuffer { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A uniform buffer bound with a dynamic offset, e.g. one large buffer
    /// holding many objects'/elements' data, rebound at a different offset
    /// via `set_bind_group`'s dynamic offsets slice instead of a bind group
    /// per object/dispatch. `element_size` is the size in bytes of a single
    /// element (before alignment padding). Use
    /// [`DynamicBufferBuilder`](crate::wgpu::buffers::DynamicBufferBuilder)
    /// to allocate the backing buffer and
    /// [`BindGroupBuilder::dynamic_buffer`](crate::wgpu::buffers::BindGroupBuilder::dynamic_buffer)
    /// (not `.buffer()`/`buffer.as_entire_binding()`) to bind it — the entry
    /// must be scoped to one element's size, not the whole buffer, or
    /// dynamic offsets will fail validation.
    pub fn dynamic_uniform_buffer(visibility: wgpu::ShaderStages, element_size: u64) -> Self {
        Self::UniformBuffer {
            visibility,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(element_size),
        }
    }

    /// A read-only storage buffer, bound as a whole (no dynamic offset).
    pub fn storage_buffer_read_only(visibility: wgpu::ShaderStages) -> Self {
        Self::StorageBufferReadOnly { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A read-write storage buffer, bound as a whole (no dynamic offset).
    pub fn storage_buffer_read_write(visibility: wgpu::ShaderStages) -> Self {
        Self::StorageBufferReadWrite { visibility, has_dynamic_offset: false, min_binding_size: None }
    }

    /// A storage buffer bound with a dynamic offset. See
    /// [`Self::dynamic_uniform_buffer`].
    pub fn dynamic_storage_buffer(visibility: wgpu::ShaderStages, element_size: u64, read_only: bool) -> Self {
        let has_dynamic_offset = true;
        let min_binding_size = wgpu::BufferSize::new(element_size);
        if read_only {
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size }
        } else {
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size }
        }
    }

    /// Which shader stage(s) this binding is visible to.
    pub fn visibility(&self) -> wgpu::ShaderStages {
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

    pub fn layout_entry(&self, binding: u32) -> wgpu::BindGroupLayoutEntry {
        match self {
            Self::Texture { visibility, sample_type, view_dimension, multisampled } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Texture {
                    sample_type: *sample_type,
                    view_dimension: *view_dimension,
                    multisampled: *multisampled,
                },
                count: None,
            },
            Self::StorageTexture { visibility, format, access, view_dimension } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::StorageTexture {
                    access: *access,
                    format: *format,
                    view_dimension: *view_dimension,
                },
                count: None,
            },
            Self::Sampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            Self::ComparisonSampler { visibility } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            Self::UniformBuffer { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            Self::StorageBufferReadOnly { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
            Self::StorageBufferReadWrite { visibility, has_dynamic_offset, min_binding_size } => wgpu::BindGroupLayoutEntry {
                binding,
                visibility: *visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: *has_dynamic_offset,
                    min_binding_size: *min_binding_size,
                },
                count: None,
            },
        }
    }
}

/// A `wgpu::BindGroupLayout`, opaque — built only via
/// [`BindGroupLayoutBuilder::build`]. There's no way to reach the underlying
/// `wgpu::BindGroupLayout` from outside this crate. `Clone` because
/// `MaterialDescriptor::extra_layouts` takes ownership (e.g. a camera's
/// layout, wired into more than one material) — cheap, the same `Arc`-backed
/// handle underneath.
#[derive(Clone)]
pub struct BindGroupLayout(wgpu::BindGroupLayout);

impl BindGroupLayout {
    pub(crate) fn raw(&self) -> &wgpu::BindGroupLayout {
        &self.0
    }
}

/// One binding within a material's or compute pass's own bind group (see
/// `MaterialDescriptor::entries`/`ComputeDescriptor::entries`).
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
///     .label("camera_layout")
///     .entry("camera", 0, BindingKind::uniform_buffer(wgpu::ShaderStages::VERTEX))
///     .build(&device);
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

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Appends one entry. Call repeatedly for a multi-entry layout.
    pub fn entry(mut self, name: &'static str, binding: u32, kind: BindingKind) -> Self {
        self.entries.push(BindingEntry { name, binding, kind });
        self
    }

    /// Appends every entry from `entries` — for building from an
    /// already-collected `Vec<BindingEntry>` (e.g.
    /// `MaterialDescriptor::entries`) rather than one at a time.
    pub fn entries(mut self, entries: impl IntoIterator<Item = BindingEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> BindGroupLayout {
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
        let stages = wgpu::ShaderStages::VERTEX_FRAGMENT;
        assert_eq!(BindingKind::texture_2d(stages).visibility(), stages);
        assert_eq!(BindingKind::sampler(stages).visibility(), stages);
        assert_eq!(BindingKind::uniform_buffer(stages).visibility(), stages);
        assert_eq!(
            BindingKind::storage_buffer_read_only(wgpu::ShaderStages::COMPUTE).visibility(),
            wgpu::ShaderStages::COMPUTE
        );
        assert_eq!(
            BindingKind::storage_buffer_read_write(wgpu::ShaderStages::COMPUTE).visibility(),
            wgpu::ShaderStages::COMPUTE
        );
    }

    #[test]
    fn dynamic_storage_buffer_picks_read_only_or_read_write_by_flag() {
        let read_only = BindingKind::dynamic_storage_buffer(wgpu::ShaderStages::COMPUTE, 16, true);
        let read_write = BindingKind::dynamic_storage_buffer(wgpu::ShaderStages::COMPUTE, 16, false);
        assert!(matches!(read_only, BindingKind::StorageBufferReadOnly { .. }));
        assert!(matches!(read_write, BindingKind::StorageBufferReadWrite { .. }));
    }

    // Device-dependent — see `test_util` for why these skip instead of
    // failing when no adapter is available.

    #[test]
    fn unique_bindings_build_without_panicking() {
        crate::wgpu::test_util::with_device!(device, _queue, {
            BindGroupLayoutBuilder::new()
                .entry("a", 0, BindingKind::texture_2d(wgpu::ShaderStages::FRAGMENT))
                .entry("b", 1, BindingKind::sampler(wgpu::ShaderStages::FRAGMENT))
                .build(&device);
        });
    }

    #[test]
    fn two_entries_claiming_the_same_binding_panics() {
        crate::wgpu::test_util::with_device!(device, _queue, {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                BindGroupLayoutBuilder::new()
                    .entry("a", 0, BindingKind::texture_2d(wgpu::ShaderStages::FRAGMENT))
                    .entry("b", 0, BindingKind::sampler(wgpu::ShaderStages::FRAGMENT))
                    .build(&device);
            }));
            assert!(result.is_err(), "expected a panic for a duplicate @binding(0)");
        });
    }
}
