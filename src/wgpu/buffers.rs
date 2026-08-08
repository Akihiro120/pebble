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
use crate::wgpu::flags::BufferUsages;
use crate::wgpu::gpu_context::GpuContext;
use crate::wgpu::samplers::Sampler;
use crate::wgpu::texture_array::GPUTextureArray;
use crate::wgpu::texture_view::TextureView;
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

impl<'a> BufferContents<'a> {
    fn size(&self) -> u64 {
        match self {
            BufferContents::Empty(size) => *size,
            BufferContents::Data(data) => data.len() as u64,
        }
    }
}

/// Builds a [`Buffer`] — empty (via [`empty`](Self::empty)) or pre-populated
/// (via [`with_data`](Self::with_data)). Two constructors rather than one
/// `new()` plus a `.data()`/`.size()` setter pair, so there's no way to call
/// both and have the second one silently win — the same shape as
/// [`Texture`](super::textures::Texture)'s `from_file`/`from_data`/`empty`.
///
/// ```ignore
/// let camera_buffer = BufferBuilder::empty(64)
///     .label("camera")
///     .uniform()
///     .build(&backend);
///
/// let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(&vertices))
///     .label("mesh vertices")
///     .usage(BufferUsages::VERTEX)
///     .build(&backend);
/// ```
///
/// For a dynamically-offset buffer (many elements, selected via
/// `set_bind_group`'s dynamic offset), use [`DynamicBufferBuilder`] instead
/// — it returns the per-element stride alongside the buffer, which plain
/// `BufferBuilder` has no way to compute.
pub struct BufferBuilder<'a> {
    label: Option<&'a str>,
    usage: BufferUsages,
    contents: BufferContents<'a>,
}

impl<'a> BufferBuilder<'a> {
    /// Allocates an empty buffer of `size` bytes, to be written into later
    /// via [`Buffer::write`].
    pub fn empty(size: u64) -> Self {
        Self { label: None, usage: BufferUsages::empty(), contents: BufferContents::Empty(size) }
    }

    /// Pre-populates the buffer with `data` (its size is taken from `data`'s length).
    pub fn with_data(data: &'a [u8]) -> Self {
        Self { label: None, usage: BufferUsages::empty(), contents: BufferContents::Data(data) }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets the buffer's usage flags outright — use this for anything not
    /// covered by [`uniform`](Self::uniform)/[`storage`](Self::storage)
    /// (a vertex/index buffer, a `MAP_READ` staging buffer, ...).
    pub fn usage(mut self, usage: BufferUsages) -> Self {
        self.usage = usage;
        self
    }

    /// Shorthand for `.usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST)`.
    pub fn uniform(self) -> Self {
        self.usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST)
    }

    /// Shorthand for `.usage(BufferUsages::STORAGE | BufferUsages::COPY_DST)`.
    pub fn storage(self) -> Self {
        self.usage(BufferUsages::STORAGE | BufferUsages::COPY_DST)
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
        check_buffer_size(device, self.label, self.usage, self.contents.size());
        match self.contents {
            BufferContents::Data(data) => {
                use wgpu::util::DeviceExt;
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: self.label,
                    contents: data,
                    usage: self.usage.into(),
                })
            }
            BufferContents::Empty(size) => device.create_buffer(&wgpu::BufferDescriptor {
                label: self.label,
                size,
                usage: self.usage.into(),
                mapped_at_creation: false,
            }),
        }
    }
}

/// Panics if `size` exceeds this device's `max_buffer_size`, or — when `usage` includes
/// `UNIFORM`/`STORAGE` — the tighter `max_uniform_buffer_binding_size`/
/// `max_storage_buffer_binding_size` those binding types are further capped to. The
/// difference between a clear message here (the actual size and the device's real limit) and
/// an opaque wgpu validation panic deep inside `create_buffer`/`create_buffer_init`.
fn check_buffer_size(device: &wgpu::Device, label: Option<&str>, usage: BufferUsages, size: u64) {
    let limits = device.limits();
    let labeled = || label.map(|l| format!(" '{l}'")).unwrap_or_default();
    if size > limits.max_buffer_size {
        panic!("buffer{}: {size} bytes exceeds this device's max_buffer_size ({})", labeled(), limits.max_buffer_size);
    }
    if usage.contains(BufferUsages::UNIFORM) && size > limits.max_uniform_buffer_binding_size {
        panic!(
            "buffer{}: {size} bytes exceeds this device's max_uniform_buffer_binding_size ({})",
            labeled(),
            limits.max_uniform_buffer_binding_size
        );
    }
    if usage.contains(BufferUsages::STORAGE) && size > limits.max_storage_buffer_binding_size {
        panic!(
            "buffer{}: {size} bytes exceeds this device's max_storage_buffer_binding_size ({})",
            labeled(),
            limits.max_storage_buffer_binding_size
        );
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
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                dynamic_uniform_offset_stride(backend, self.element_size),
            ),
            DynamicKind::Storage => (
                BufferUsages::STORAGE | BufferUsages::COPY_DST,
                dynamic_storage_offset_stride(backend, self.element_size),
            ),
        };
        let buffer = BufferBuilder::empty(stride * self.count).label(self.label).usage(usage).build(backend);
        DynamicBuffer::new(buffer, stride, self.element_size)
    }
}

/// Rounds `element_size` up to the device's required alignment for dynamic offsets on
/// uniform buffers, giving the stride to use when packing multiple elements into one
/// buffer for use with [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer).
/// [`DynamicBufferBuilder`] calls this for you — use it directly only if you're sizing
/// a dynamic buffer some other way.
pub fn dynamic_uniform_offset_stride(backend: &WGPUBackend, element_size: u64) -> u64 {
    dynamic_uniform_offset_stride_raw(&backend.device, element_size)
}

/// Internal primitive behind [`dynamic_uniform_offset_stride`] — used
/// directly only by tests, which have a raw `wgpu::Device` but no full
/// [`WGPUBackend`].
pub(crate) fn dynamic_uniform_offset_stride_raw(device: &wgpu::Device, element_size: u64) -> u64 {
    align_to(element_size, device.limits().min_uniform_buffer_offset_alignment as u64)
}

/// Same as [`dynamic_uniform_offset_stride`] but for storage buffers.
pub fn dynamic_storage_offset_stride(backend: &WGPUBackend, element_size: u64) -> u64 {
    dynamic_storage_offset_stride_raw(&backend.device, element_size)
}

/// Internal primitive behind [`dynamic_storage_offset_stride`] — used
/// directly only by tests, which have a raw `wgpu::Device` but no full
/// [`WGPUBackend`].
pub(crate) fn dynamic_storage_offset_stride_raw(device: &wgpu::Device, element_size: u64) -> u64 {
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
///     .build(&backend);
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
        self.texture_view_raw_at(binding, texture.view())
    }

    /// Binds a texture array's view at the next `@binding(N)`.
    pub fn texture_array(self, texture: &'a GPUTextureArray) -> Self {
        let binding = self.next_binding;
        self.texture_array_at(binding, texture)
    }

    /// Same as [`texture_array`](Self::texture_array) but at an explicit `@binding(N)`.
    pub fn texture_array_at(self, binding: u32, texture: &'a GPUTextureArray) -> Self {
        self.texture_view_raw_at(binding, texture.view())
    }

    /// Binds a cubemap's view at the next `@binding(N)`.
    pub fn texture_cubemap(self, texture: &'a GPUCubemap) -> Self {
        let binding = self.next_binding;
        self.texture_cubemap_at(binding, texture)
    }

    /// Same as [`texture_cubemap`](Self::texture_cubemap) but at an explicit `@binding(N)`.
    pub fn texture_cubemap_at(self, binding: u32, texture: &'a GPUCubemap) -> Self {
        self.texture_view_raw_at(binding, texture.view())
    }

    /// Binds an opaque [`TextureView`] — a render target built via
    /// [`RenderTargetTextureBuilder`](super::texture_view::RenderTargetTextureBuilder)/
    /// [`GPUCubemap::face_attachment`](GPUCubemap::face_attachment) — at the
    /// next `@binding(N)`, for sampling it back in a later pass (a shadow
    /// map, a post-process input, ...).
    pub fn texture_view(self, view: &'a TextureView) -> Self {
        let binding = self.next_binding;
        self.texture_view_at(binding, view)
    }

    /// Same as [`texture_view`](Self::texture_view) but at an explicit `@binding(N)`.
    pub fn texture_view_at(self, binding: u32, view: &'a TextureView) -> Self {
        self.texture_view_raw_at(binding, view.raw())
    }

    /// Low-level primitive behind every `texture_*` method above — kept
    /// `pub(crate)` for internal code (mipmap generation's blit pass) that
    /// binds an ad-hoc single-mip-level view rather than a whole
    /// [`GPUTexture`]/[`GPUTextureArray`]/[`GPUCubemap`]/[`TextureView`].
    pub(crate) fn texture_view_raw_at(mut self, binding: u32, view: &'a wgpu::TextureView) -> Self {
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
    /// [`texture_view_raw_at`](Self::texture_view_raw_at).
    pub(crate) fn sampler_raw_at(mut self, binding: u32, sampler: &'a wgpu::Sampler) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::Sampler(sampler) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn build(self, backend: &WGPUBackend) -> BindGroup {
        BindGroup(self.build_raw(&backend.device))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::test_util::with_device;

    #[test]
    fn a_size_within_every_limit_does_not_panic() {
        with_device!(device, _queue, {
            BufferBuilder::empty(64).uniform().build_raw(&device);
        });
    }

    #[test]
    fn exceeding_max_buffer_size_panics() {
        with_device!(device, _queue, {
            let too_big = device.limits().max_buffer_size + 1;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                BufferBuilder::empty(too_big).usage(BufferUsages::COPY_DST).build_raw(&device);
            }));
            assert!(result.is_err(), "expected a panic for a size exceeding max_buffer_size");
        });
    }

    #[test]
    fn exceeding_max_uniform_buffer_binding_size_panics() {
        with_device!(device, _queue, {
            let too_big = device.limits().max_uniform_buffer_binding_size + 1;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                BufferBuilder::empty(too_big).uniform().build_raw(&device);
            }));
            assert!(
                result.is_err(),
                "expected a panic for a size exceeding max_uniform_buffer_binding_size"
            );
        });
    }

    #[test]
    fn exceeding_max_uniform_buffer_binding_size_is_fine_for_a_non_uniform_buffer() {
        // Same size that panics as a uniform buffer above must NOT panic here — the tighter
        // check only applies when `usage` actually includes UNIFORM (not STORAGE either, so
        // this can't accidentally trip the sibling max_storage_buffer_binding_size check).
        with_device!(device, _queue, {
            let big = (device.limits().max_uniform_buffer_binding_size + 1).min(device.limits().max_buffer_size);
            BufferBuilder::empty(big).usage(BufferUsages::COPY_DST).build_raw(&device);
        });
    }
}
