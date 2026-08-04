//! Buffer and bind-group construction — three builders, one per thing being
//! built:
//! - [`BufferBuilder`] — a plain, uniform, or storage buffer, empty or
//!   pre-populated with data.
//! - [`DynamicBufferBuilder`] — a buffer sized and aligned to hold many
//!   dynamically-offset elements; returns the buffer *and* the per-element
//!   stride you'll need later, so the two can't drift apart.
//! - [`BindGroupBuilder`] — assembles a `wgpu::BindGroup` from already-built
//!   buffers/texture views/samplers, one binding at a time.
//!
//! Prefer these over hand-writing `wgpu::BufferDescriptor`/`BindGroupDescriptor`
//! against `backend.device` directly: correct usage flags are one method
//! call away instead of memorized flag combinations, and the dynamic-offset
//! path gets alignment right in a way that's easy to miss by hand. Re-exported,
//! along with [`binding`](super::binding), from [`wgpu::prelude`](super::prelude).

// ---------------------------------------------------------------------
// Plain buffers
// ---------------------------------------------------------------------

enum BufferContents<'a> {
    Empty(u64),
    Data(&'a [u8]),
}

/// Builds a `wgpu::Buffer` — empty (via [`size`](Self::size)) or
/// pre-populated (via [`data`](Self::data)).
///
/// ```ignore
/// let camera_buffer = BufferBuilder::new()
///     .label("camera")
///     .uniform()
///     .size(64)
///     .build(&device);
///
/// let vertex_buffer = BufferBuilder::new()
///     .label("mesh vertices")
///     .usage(wgpu::BufferUsages::VERTEX)
///     .data(bytemuck::cast_slice(&vertices))
///     .build(&device);
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
    /// (e.g. via [`update_buffer`]). Mutually exclusive with
    /// [`data`](Self::data) — whichever is called last wins.
    pub fn size(mut self, size: u64) -> Self {
        self.contents = BufferContents::Empty(size);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> wgpu::Buffer {
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
// Writing to an existing buffer
// ---------------------------------------------------------------------

/// Overwrites `buffer`'s contents with `data`, starting at offset 0. Plain
/// `queue.write_buffer` underneath — works for any buffer usage, not just
/// uniform buffers, despite the neighboring [`update_buffer_at`]'s
/// dynamic-offset framing. Not a builder: there's nothing optional here to
/// configure, just an existing buffer to write into.
pub fn update_buffer(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &[u8]) {
    queue.write_buffer(buffer, 0, data);
}

/// Writes `data` into `buffer` at a byte offset, for updating one element of a
/// dynamically-offset buffer without touching the others. `offset` should be a
/// multiple of the stride returned by [`DynamicBufferBuilder::build`].
pub fn update_buffer_at(queue: &wgpu::Queue, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
    queue.write_buffer(buffer, offset, data);
}

// ---------------------------------------------------------------------
// Dynamically-offset buffers
// ---------------------------------------------------------------------

enum DynamicKind {
    Uniform,
    Storage,
}

/// Builds an empty buffer sized and aligned to hold `count` dynamically-offset
/// elements of `element_size` bytes each — for one large buffer holding many
/// objects'/elements' data, rebound at a different offset via
/// `set_bind_group`'s dynamic offsets slice instead of a bind group per
/// object/dispatch. Pair with a layout entry from
/// [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer)/
/// [`dynamic_storage_buffer`](super::binding::BindingKind::dynamic_storage_buffer).
///
/// ```ignore
/// let (buffer, stride) = DynamicBufferBuilder::uniform(element_size, count).build(&device);
/// // ... later, per element:
/// update_buffer_at(&queue, &buffer, index as u64 * stride, &element_bytes);
/// // ... at draw time:
/// pass.set_bind_group(0, Some(&bind_group), &[index as u32 * stride as u32]);
/// ```
///
/// [`build`](Self::build) returns `(wgpu::Buffer, u64)` — the buffer and the
/// per-element stride to use for both `update_buffer_at` and
/// `set_bind_group`'s dynamic offset — rather than [`BufferBuilder`]'s plain
/// `wgpu::Buffer`, since there'd otherwise be no way to recover the
/// alignment-padded stride after the fact.
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

    pub fn build(self, device: &wgpu::Device) -> (wgpu::Buffer, u64) {
        let (usage, stride) = match self.kind {
            DynamicKind::Uniform => (
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                dynamic_uniform_offset_stride(device, self.element_size),
            ),
            DynamicKind::Storage => (
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                dynamic_storage_offset_stride(device, self.element_size),
            ),
        };
        let buffer = BufferBuilder::new()
            .label(self.label)
            .usage(usage)
            .size(stride * self.count)
            .build(device);
        (buffer, stride)
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
/// The plain methods ([`buffer`](Self::buffer), [`texture`](Self::texture),
/// [`sampler`](Self::sampler), [`dynamic_buffer`](Self::dynamic_buffer))
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
    pub fn new(layout: &'a wgpu::BindGroupLayout) -> Self {
        Self { label: None, layout, entries: Vec::new(), next_binding: 0 }
    }

    pub fn label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    /// Binds `buffer` in its entirety at the next `@binding(N)` (call order,
    /// starting at 0).
    pub fn buffer(self, buffer: &'a wgpu::Buffer) -> Self {
        let binding = self.next_binding;
        self.buffer_at(binding, buffer)
    }

    /// Same as [`buffer`](Self::buffer) but at an explicit `@binding(N)`.
    pub fn buffer_at(mut self, binding: u32, buffer: &'a wgpu::Buffer) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: buffer.as_entire_binding() });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds `buffer` scoped to one `element_size`-sized element — not the
    /// whole buffer, since the dynamic offset passed to `set_bind_group` at
    /// draw/dispatch time is added on top of this base range, and wgpu
    /// validates `offset + size <= buffer size` — at the next `@binding(N)`,
    /// for a buffer built by [`DynamicBufferBuilder`].
    pub fn dynamic_buffer(self, buffer: &'a wgpu::Buffer, element_size: u64) -> Self {
        let binding = self.next_binding;
        self.dynamic_buffer_at(binding, buffer, element_size)
    }

    /// Same as [`dynamic_buffer`](Self::dynamic_buffer) but at an explicit `@binding(N)`.
    pub fn dynamic_buffer_at(mut self, binding: u32, buffer: &'a wgpu::Buffer, element_size: u64) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: dynamic_buffer_binding(buffer, element_size) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds `view` at the next `@binding(N)`.
    pub fn texture(self, view: &'a wgpu::TextureView) -> Self {
        let binding = self.next_binding;
        self.texture_at(binding, view)
    }

    /// Same as [`texture`](Self::texture) but at an explicit `@binding(N)`.
    pub fn texture_at(mut self, binding: u32, view: &'a wgpu::TextureView) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(view) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    /// Binds `sampler` at the next `@binding(N)`.
    pub fn sampler(self, sampler: &'a wgpu::Sampler) -> Self {
        let binding = self.next_binding;
        self.sampler_at(binding, sampler)
    }

    /// Same as [`sampler`](Self::sampler) but at an explicit `@binding(N)`.
    pub fn sampler_at(mut self, binding: u32, sampler: &'a wgpu::Sampler) -> Self {
        self.entries.push(wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::Sampler(sampler) });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn build(self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: self.label,
            layout: self.layout,
            entries: &self.entries,
        })
    }
}
