//! Buffer and bind-group construction — three builders, one per thing being
//! built:
//! - [`BufferBuilder`] — a plain, uniform, or storage [`Buffer`], empty or
//!   pre-populated with data.
//! - [`DynamicBufferBuilder`] — a [`DynamicBuffer`] sized to hold many
//!   dynamically-offset elements; bundles the per-element stride so it
//!   can't drift out of sync with what the buffer was actually built with.
//! - [`BindGroupBuilder`] — assembles a `wgpu::BindGroup` from already-built
//!   [`Buffer`]s/textures/samplers, one binding at a time.
//!
//! Prefer these over hand-writing `wgpu::BufferDescriptor`/`BindGroupDescriptor`
//! against `backend.device` directly: correct usage flags are one method
//! call away instead of memorized flag combinations, and the dynamic-offset
//! path gets alignment right in a way that's easy to miss by hand. Re-exported,
//! along with [`binding`](super::binding), from [`wgpu::prelude`](super::prelude).

use crate::wgpu::backend::WGPUBackend;
use crate::wgpu::binding::BindGroupLayout;
use crate::wgpu::buffer::{Buffer, DynamicBuffer};
use crate::wgpu::cubemap::GPUCubemap;
use crate::wgpu::gpu_context::GpuContext;
use crate::wgpu::samplers::Sampler;
use crate::wgpu::texture_array::GPUTextureArray;
use crate::wgpu::textures::GPUTexture;

/// A `wgpu::BindGroup`, opaque — built only via [`BindGroupBuilder::build`].
/// Bind it against a [`RenderPass`](super::render_pass::RenderPass)/
/// [`ComputePass`](super::compute_pass::ComputePass) via their
/// `set_bind_group`; there's no way to reach the underlying `wgpu::BindGroup`
/// from outside this crate.
pub struct BindGroup(wgpu::BindGroup);

impl BindGroup {
    pub(crate) fn raw(&self) -> &wgpu::BindGroup {
        &self.0
    }
}

// ---------------------------------------------------------------------
// Plain buffers
// ---------------------------------------------------------------------

enum BufferContents<'a> {
    Empty(u64),
    Data(&'a [u8]),
}

/// Builds a [`Buffer`] — empty (via [`size`](Self::size)) or pre-populated
/// (via [`data`](Self::data)).
///
/// ```ignore
/// let camera_buffer = BufferBuilder::new()
///     .label("camera")
///     .uniform()
///     .size(64)
///     .build(&backend);
///
/// let vertex_buffer = BufferBuilder::new()
///     .label("mesh vertices")
///     .usage(wgpu::BufferUsages::VERTEX)
///     .data(bytemuck::cast_slice(&vertices))
///     .build(&backend);
/// ```
///
/// For a dynamically-offset buffer (many elements, selected via
/// `set_bind_group`'s dynamic offset), use [`DynamicBufferBuilder`] instead
/// — it returns the per-element stride alongside the buffer, which plain
/// `BufferBuilder` has no way to compute.
pub struct BufferBuilder<'a> {
    label: Option<&'a str>,
    usage: wgpu::BufferUsages,
    contents: BufferContents<'a>,
}

impl<'a> Default for BufferBuilder<'a> {
    fn default() -> Self {
        Self { label: None, usage: wgpu::BufferUsages::empty(), contents: BufferContents::Empty(0) }
    }
}

impl<'a> BufferBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets the buffer's usage flags outright — use this for anything not
    /// covered by [`uniform`](Self::uniform)/[`storage`](Self::storage)
    /// (a vertex/index buffer, a `MAP_READ` staging buffer, ...).
    pub fn usage(mut self, usage: wgpu::BufferUsages) -> Self {
        self.usage = usage;
        self
    }

    /// Shorthand for `.usage(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)`.
    pub fn uniform(self) -> Self {
        self.usage(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
    }

    /// Shorthand for `.usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)`.
    pub fn storage(self) -> Self {
        self.usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
    }

    /// Pre-populates the buffer with `data` (its size is taken from `data`'s
    /// length). Mutually exclusive with [`size`](Self::size) — whichever is
    /// called last wins.
    pub fn data(mut self, data: &'a [u8]) -> Self {
        self.contents = BufferContents::Data(data);
        self
    }

    /// Allocates an empty buffer of `size` bytes, to be written into later
    /// via [`Buffer::write`]. Mutually exclusive with [`data`](Self::data) —
    /// whichever is called last wins.
    pub fn size(mut self, size: u64) -> Self {
        self.contents = BufferContents::Empty(size);
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> Buffer {
        let raw = self.build_raw(&backend.device);
        Buffer::new(raw, GpuContext::from_backend(backend))
    }

    /// Internal primitive behind [`build`](Self::build) — used directly only
    /// where a [`WGPUBackend`] isn't available yet (bootstrapping a staging
    /// buffer for [`Buffer::read`](crate::wgpu::buffer::Buffer::read), which
    /// only has `device`/`queue` separately).
    pub(crate) fn build_raw(self, device: &wgpu::Device) -> wgpu::Buffer {
        match self.contents {
            BufferContents::Data(data) => {
                use wgpu::util::DeviceExt;
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: self.label,
                    contents: data,
                    usage: self.usage,
                })
            }
            BufferContents::Empty(size) => device.create_buffer(&wgpu::BufferDescriptor {
                label: self.label,
                size,
                usage: self.usage,
                mapped_at_creation: false,
            }),
        }
    }
}

// ---------------------------------------------------------------------
// Dynamically-offset buffers
// ---------------------------------------------------------------------

enum DynamicKind {
    Uniform,
    Storage,
}

/// Builds a [`DynamicBuffer`] — empty, sized and aligned to hold `count`
/// dynamically-offset elements of `element_size` bytes each — for one large
/// buffer holding many objects'/elements' data, rebound at a different
/// offset via `set_bind_group`'s dynamic offsets slice instead of a bind
/// group per object/dispatch. Pair with a layout entry from
/// [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer)/
/// [`dynamic_storage_buffer`](super::binding::BindingKind::dynamic_storage_buffer).
///
/// ```ignore
/// let dynamic = DynamicBufferBuilder::uniform(element_size, count).build(&backend);
/// // ... later, per element:
/// dynamic.write_element(index, &element_bytes);
/// // ... at draw time:
/// pass.set_bind_group(0, Some(&bind_group), &[index as u32 * dynamic.stride() as u32]);
/// ```
pub struct DynamicBufferBuilder<'a> {
    label: Option<&'a str>,
    kind: DynamicKind,
    element_size: u64,
    count: u64,
}

impl<'a> DynamicBufferBuilder<'a> {
    pub fn uniform(element_size: u64, count: u64) -> Self {
        Self { label: None, kind: DynamicKind::Uniform, element_size, count }
    }

    pub fn storage(element_size: u64, count: u64) -> Self {
        Self { label: None, kind: DynamicKind::Storage, element_size, count }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> DynamicBuffer {
        let (usage, stride) = match self.kind {
            DynamicKind::Uniform => (
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                dynamic_uniform_offset_stride(&backend.device, self.element_size),
            ),
            DynamicKind::Storage => (
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                dynamic_storage_offset_stride(&backend.device, self.element_size),
            ),
        };
        let buffer = BufferBuilder::new()
            .label(self.label)
            .usage(usage)
            .size(stride * self.count)
            .build(backend);
        DynamicBuffer::new(buffer, stride, self.element_size)
    }
}

/// Rounds `element_size` up to the device's required alignment for dynamic offsets on
/// uniform buffers, giving the stride to use when packing multiple elements into one
/// buffer for use with [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer).
/// [`DynamicBufferBuilder`] calls this for you — use it directly only if you're sizing
/// a dynamic buffer some other way.
pub fn dynamic_uniform_offset_stride(device: &wgpu::Device, element_size: u64) -> u64 {
    align_to(element_size, device.limits().min_uniform_buffer_offset_alignment as u64)
}

/// Same as [`dynamic_uniform_offset_stride`] but for storage buffers.
pub fn dynamic_storage_offset_stride(device: &wgpu::Device, element_size: u64) -> u64 {
    align_to(element_size, device.limits().min_storage_buffer_offset_alignment as u64)
}

fn align_to(size: u64, alignment: u64) -> u64 {
    size.div_ceil(alignment) * alignment
}

/// Builds the bind group entry resource for a dynamically-offset binding. Unlike
/// `buffer.as_entire_binding()`, this scopes the entry to a single `element_size`-sized
/// element starting at offset 0 in the buffer — required because the dynamic offset passed
/// to `set_bind_group` at draw/dispatch time is added on top of this base range, and wgpu
/// validates `offset + size <= buffer size`. Binding the whole buffer here would make any
/// nonzero dynamic offset fail validation. [`BindGroupBuilder::dynamic_buffer`] calls this
/// for you.
fn dynamic_buffer_binding(buffer: &wgpu::Buffer, element_size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: wgpu::BufferSize::new(element_size),
    })
}

// ---------------------------------------------------------------------
// Bind groups
// ---------------------------------------------------------------------

/// Builds a `wgpu::BindGroup` against `layout` one binding at a time.
///
/// The plain methods ([`buffer`](Self::buffer), [`texture_2d`](Self::texture_2d),
/// [`sampler`](Self::sampler), [`dynamic_buffer`](Self::dynamic_buffer), ...)
/// assign `@binding(N)` in call order, starting at 0 — the common case,
/// matching a layout whose entries are numbered the same way. If your
/// target's bindings aren't contiguous from 0 (e.g. looked up by name
/// against a [`BindGroupTarget`](super::binding::BindGroupTarget), as
/// [`GPUBindingInstance`](super::instance::GPUBindingInstance) does), use
/// the `_at` variants to assign an explicit `@binding(N)` instead.
///
/// ```ignore
/// let bind_group = BindGroupBuilder::new(&layout)
///     .label("camera_bind_group")
///     .buffer(&camera_buffer)
///     .build(&device);
/// ```
pub struct BindGroupBuilder<'a> {
    label: Option<&'a str>,
    layout: &'a wgpu::BindGroupLayout,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
    next_binding: u32,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new(layout: &'a BindGroupLayout) -> Self {
        Self::new_raw(layout.raw())
    }

    /// Internal primitive behind [`new`](Self::new) — used directly only by
    /// code with its own raw `wgpu::BindGroupLayout` that never goes through
    /// [`BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder)
    /// (mipmap generation's fixed-shape blit layout).
    pub(crate) fn new_raw(layout: &'a wgpu::BindGroupLayout) -> Self {
        Self { label: None, layout, entries: Vec::new(), next_binding: 0 }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Binds `buffer` in its entirety at the next `@binding(N)` (call order,
    /// starting at 0).
    pub fn buffer(self, buffer: &'a Buffer) -> Self {
        let binding = self.next_binding;
        self.buffer_at(binding, buffer)
    }

    /// Same as [`buffer`](Self::buffer) but at an explicit `@binding(N)`.
    pub fn buffer_at(mut self, binding: u32, buffer: &'a Buffer) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: buffer.raw().as_entire_binding() });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds `buffer` scoped to one element (see [`DynamicBuffer::element_size`])
    /// at the next `@binding(N)`.
    pub fn dynamic_buffer(self, buffer: &'a DynamicBuffer) -> Self {
        let binding = self.next_binding;
        self.dynamic_buffer_at(binding, buffer)
    }

    /// Same as [`dynamic_buffer`](Self::dynamic_buffer) but at an explicit `@binding(N)`.
    pub fn dynamic_buffer_at(mut self, binding: u32, buffer: &'a DynamicBuffer) -> Self {
        let resource = dynamic_buffer_binding(buffer.buffer.raw(), buffer.element_size);
        self.entries.push(wgpu::BindGroupEntry { binding, resource });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds a 2D texture's view at the next `@binding(N)`.
    pub fn texture_2d(self, texture: &'a GPUTexture) -> Self {
        let binding = self.next_binding;
        self.texture_2d_at(binding, texture)
    }

    /// Same as [`texture_2d`](Self::texture_2d) but at an explicit `@binding(N)`.
    pub fn texture_2d_at(self, binding: u32, texture: &'a GPUTexture) -> Self {
        self.texture_view_at(binding, texture.view())
    }

    /// Binds a texture array's view at the next `@binding(N)`.
    pub fn texture_array(self, texture: &'a GPUTextureArray) -> Self {
        let binding = self.next_binding;
        self.texture_array_at(binding, texture)
    }

    /// Same as [`texture_array`](Self::texture_array) but at an explicit `@binding(N)`.
    pub fn texture_array_at(self, binding: u32, texture: &'a GPUTextureArray) -> Self {
        self.texture_view_at(binding, texture.view())
    }

    /// Binds a cubemap's view at the next `@binding(N)`.
    pub fn texture_cubemap(self, texture: &'a GPUCubemap) -> Self {
        let binding = self.next_binding;
        self.texture_cubemap_at(binding, texture)
    }

    /// Same as [`texture_cubemap`](Self::texture_cubemap) but at an explicit `@binding(N)`.
    pub fn texture_cubemap_at(self, binding: u32, texture: &'a GPUCubemap) -> Self {
        self.texture_view_at(binding, texture.view())
    }

    /// Low-level primitive behind the `texture_*` methods above — kept
    /// `pub(crate)` for internal code (mipmap generation's blit pass) that
    /// binds an ad-hoc single-mip-level view rather than a whole
    /// [`GPUTexture`]/[`GPUTextureArray`]/[`GPUCubemap`].
    pub(crate) fn texture_view_at(mut self, binding: u32, view: &'a wgpu::TextureView) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds `sampler` at the next `@binding(N)`.
    pub fn sampler(self, sampler: &'a Sampler) -> Self {
        let binding = self.next_binding;
        self.sampler_at(binding, sampler)
    }

    /// Same as [`sampler`](Self::sampler) but at an explicit `@binding(N)`.
    pub fn sampler_at(self, binding: u32, sampler: &'a Sampler) -> Self {
        self.sampler_raw_at(binding, sampler.raw())
    }

    /// Low-level primitive behind [`sampler`](Self::sampler) — kept
    /// `pub(crate)` for the same internal reason as
    /// [`texture_view_at`](Self::texture_view_at).
    pub(crate) fn sampler_raw_at(mut self, binding: u32, sampler: &'a wgpu::Sampler) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::Sampler(sampler) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> BindGroup {
        BindGroup(self.build_raw(device))
    }

    /// Internal primitive behind [`build`](Self::build) — used directly only
    /// by code that needs a raw `wgpu::BindGroup` to feed into a raw
    /// `wgpu::RenderPass` it built itself (mipmap generation's blit pass).
    pub(crate) fn build_raw(self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: self.label,
            layout: self.layout,
            entries: &self.entries,
        })
    }
}
