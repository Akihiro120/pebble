//! Buffer and bind-group construction helpers.
//!
//! Naming convention used throughout this file:
//! - `build_*` — always allocates something new.
//! - `resolve_*` — allocates from raw bytes (`BufferSource::Data`), or
//!   passes an already-built buffer through unchanged (`BufferSource::Buffer`)
//!   — "give me *a* buffer for this, new or not" rather than "make me one."
//! - `update_*` — writes into a buffer that already exists; never allocates.
//! - `dynamic_*` — the per-element-offset counterpart of a plain helper,
//!   for packing many elements into one buffer and selecting between them
//!   with `set_bind_group`'s dynamic offset instead of a bind group each.
//!
//! Sections below, roughly simple-to-complex: fresh buffers, resolving a
//! [`BufferSource`], writing to an existing buffer, single-resource bind
//! groups, dynamic-offset helpers, then the general multi-resource
//! [`build_bind_group`] everything above is built from.

/// Either raw bytes to upload into a fresh buffer, or an already-built
/// buffer to use as-is. Accepted via `impl Into<BufferSource>` by the
/// `resolve_*`/`build_*_bind_group` functions below, so callers can pass a
/// `&[u8]` directly (the common case) without constructing this by hand.
pub enum BufferSource<'a> {
    /// Bytes to upload into a newly-created buffer.
    Data(&'a [u8]),
    /// A buffer that already exists — used as-is, no upload.
    Buffer(wgpu::Buffer),
}

impl<'a> From<&'a [u8]> for BufferSource<'a> {
    fn from(data: &'a [u8]) -> Self {
        BufferSource::Data(data)
    }
}

impl<'a> From<wgpu::Buffer> for BufferSource<'a> {
    fn from(buffer: wgpu::Buffer) -> Self {
        BufferSource::Buffer(buffer)
    }
}

/// One resource to bind into a bind group, passed to [`build_bind_group`].
/// Covers every binding kind [`BindingKind`](super::binding::BindingKind)
/// can describe.
pub enum BindingResource<'a> {
    /// A uniform buffer, built from or passed through via [`BufferSource`].
    UniformBuffer(BufferSource<'a>),
    /// A storage buffer, built from or passed through via [`BufferSource`].
    StorageBuffer(BufferSource<'a>),
    /// A uniform buffer bound with a dynamic offset. The bind group entry is scoped to a
    /// single `element_size`-sized element (see [`dynamic_buffer_binding`]) rather than the
    /// whole buffer, so it works with the dynamic offset passed to `set_bind_group`.
    DynamicUniformBuffer { buffer: wgpu::Buffer, element_size: u64 },
    /// Same as `DynamicUniformBuffer` but for a storage buffer.
    DynamicStorageBuffer { buffer: wgpu::Buffer, element_size: u64 },
    /// A texture view (e.g. for a `texture_2d` shader binding).
    TextureView(&'a wgpu::TextureView),
    /// A sampler.
    Sampler(&'a wgpu::Sampler),
}

// ---------------------------------------------------------------------
// Fresh buffers
// ---------------------------------------------------------------------

/// Creates a buffer pre-populated with `contents`.
pub fn build_buffer(device: &wgpu::Device, contents: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents,
        usage,
    })
}

/// Creates an empty buffer of `size` bytes, to be written into later (e.g.
/// via [`update_buffer`]).
pub fn build_buffer_sized(device: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage,
        mapped_at_creation: false,
    })
}

// ---------------------------------------------------------------------
// Resolving a BufferSource — build fresh from bytes, or pass an existing
// buffer through unchanged.
// ---------------------------------------------------------------------

/// Builds a fresh buffer from `Data`, or passes an already-built `Buffer`
/// straight through unchanged.
pub fn resolve_buffer(device: &wgpu::Device, source: BufferSource<'_>, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    match source {
        BufferSource::Data(data) => build_buffer(device, data, usage),
        BufferSource::Buffer(buffer) => buffer,
    }
}

/// [`resolve_buffer`] with `UNIFORM | COPY_DST` usage.
pub fn resolve_uniform_buffer(device: &wgpu::Device, source: BufferSource<'_>) -> wgpu::Buffer {
    resolve_buffer(device, source, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
}

/// [`resolve_buffer`] with `STORAGE | COPY_DST` usage.
pub fn resolve_storage_buffer(device: &wgpu::Device, source: BufferSource<'_>) -> wgpu::Buffer {
    resolve_buffer(device, source, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
}

// ---------------------------------------------------------------------
// Writing to an existing buffer
// ---------------------------------------------------------------------

/// Overwrites `buffer`'s contents with `data`, starting at offset 0. Plain
/// `queue.write_buffer` underneath — works for any buffer usage, not just
/// uniform buffers, despite the neighboring `update_buffer_at`'s
/// dynamic-offset framing.
pub fn update_buffer(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &[u8]) {
    queue.write_buffer(buffer, 0, data);
}

/// Writes `data` into `buffer` at a byte offset, for updating one element of a
/// dynamically-offset buffer without touching the others. `offset` should be a
/// multiple of the stride returned by [`dynamic_uniform_offset_stride`]
/// (or [`dynamic_storage_offset_stride`], for a storage buffer).
pub fn update_buffer_at(queue: &wgpu::Queue, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
    queue.write_buffer(buffer, offset, data);
}

// ---------------------------------------------------------------------
// Single-resource bind groups
// ---------------------------------------------------------------------

/// Resolves `source` into a uniform buffer and builds a single-entry bind
/// group for it against `layout`, returning both.
pub fn build_uniform_bind_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: impl Into<BufferSource<'a>>,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let (mut buffers, bind_group) = build_bind_group(device, layout, vec![BindingResource::UniformBuffer(source.into())]);
    (buffers.remove(0), bind_group)
}

/// Same as [`build_uniform_bind_group`] but for a storage buffer.
pub fn build_storage_bind_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: impl Into<BufferSource<'a>>,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let (mut buffers, bind_group) = build_bind_group(device, layout, vec![BindingResource::StorageBuffer(source.into())]);
    (buffers.remove(0), bind_group)
}

/// Allocates a dynamically-offset uniform buffer sized for `count` elements and builds a
/// bind group for it in one step. Returns the buffer, the per-element stride to use as the
/// dynamic offset in `set_bind_group`, and the bind group. Use with a layout built from
/// [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer).
pub fn build_dynamic_uniform_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    element_size: u64,
    count: u64,
) -> (wgpu::Buffer, u64, wgpu::BindGroup) {
    let (buffer, stride) = build_dynamic_uniform_buffer(device, element_size, count);
    let (mut buffers, bind_group) =
        build_bind_group(device, layout, vec![BindingResource::DynamicUniformBuffer { buffer, element_size }]);
    (buffers.remove(0), stride, bind_group)
}

/// Same as [`build_dynamic_uniform_bind_group`] but for a storage buffer.
pub fn build_dynamic_storage_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    element_size: u64,
    count: u64,
) -> (wgpu::Buffer, u64, wgpu::BindGroup) {
    let (buffer, stride) = build_dynamic_storage_buffer(device, element_size, count);
    let (mut buffers, bind_group) =
        build_bind_group(device, layout, vec![BindingResource::DynamicStorageBuffer { buffer, element_size }]);
    (buffers.remove(0), stride, bind_group)
}

// ---------------------------------------------------------------------
// Dynamic-offset helpers
// ---------------------------------------------------------------------

/// Rounds `element_size` up to the device's required alignment for dynamic offsets on
/// uniform buffers, giving the stride to use when packing multiple elements into one
/// buffer for use with [`BindingKind::dynamic_uniform_buffer`](super::binding::BindingKind::dynamic_uniform_buffer).
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
/// nonzero dynamic offset fail validation.
pub fn dynamic_buffer_binding(buffer: &wgpu::Buffer, element_size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: wgpu::BufferSize::new(element_size),
    })
}

/// Builds an empty buffer sized to hold `count` elements of a dynamically-offset uniform
/// buffer, and returns the buffer along with the per-element stride to use as dynamic
/// offsets in `RenderPass::set_bind_group`.
pub fn build_dynamic_uniform_buffer(device: &wgpu::Device, element_size: u64, count: u64) -> (wgpu::Buffer, u64) {
    let stride = dynamic_uniform_offset_stride(device, element_size);
    let buffer = build_buffer_sized(device, stride * count, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
    (buffer, stride)
}

/// Builds an empty buffer sized to hold `count` elements of a dynamically-offset storage
/// buffer, and returns the buffer along with the per-element stride.
pub fn build_dynamic_storage_buffer(device: &wgpu::Device, element_size: u64, count: u64) -> (wgpu::Buffer, u64) {
    let stride = dynamic_storage_offset_stride(device, element_size);
    let buffer = build_buffer_sized(device, stride * count, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
    (buffer, stride)
}

// ---------------------------------------------------------------------
// General multi-resource bind group — the primitive every build_*_bind_group
// helper above is built from. Reach for it directly when you need more than
// one binding (a texture + sampler + uniform buffer, say) in a single group.
// ---------------------------------------------------------------------

/// Builds a bind group from multiple resources. Buffers created from `Data` are returned
/// in order (texture views and samplers are not returned). Pre-built buffers passed via
/// `Buffer` are consumed and also returned.
pub fn build_bind_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    resources: Vec<BindingResource<'a>>,
) -> (Vec<wgpu::Buffer>, wgpu::BindGroup) {
    enum Resolved<'a> {
        Buffer(wgpu::Buffer),
        DynamicBuffer(wgpu::Buffer, u64),
        TextureView(&'a wgpu::TextureView),
        Sampler(&'a wgpu::Sampler),
    }

    let resolved: Vec<Resolved> = resources
        .into_iter()
        .map(|r| match r {
            BindingResource::UniformBuffer(src) => Resolved::Buffer(resolve_uniform_buffer(device, src)),
            BindingResource::StorageBuffer(src) => Resolved::Buffer(resolve_storage_buffer(device, src)),
            BindingResource::DynamicUniformBuffer { buffer, element_size } => Resolved::DynamicBuffer(buffer, element_size),
            BindingResource::DynamicStorageBuffer { buffer, element_size } => Resolved::DynamicBuffer(buffer, element_size),
            BindingResource::TextureView(view) => Resolved::TextureView(view),
            BindingResource::Sampler(sampler) => Resolved::Sampler(sampler),
        })
        .collect();

    let bind_group = {
        let entries: Vec<wgpu::BindGroupEntry> = resolved
            .iter()
            .enumerate()
            .map(|(i, r)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: match r {
                    Resolved::Buffer(buf) => buf.as_entire_binding(),
                    Resolved::DynamicBuffer(buf, element_size) => dynamic_buffer_binding(buf, *element_size),
                    Resolved::TextureView(view) => wgpu::BindingResource::TextureView(view),
                    Resolved::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
                },
            })
            .collect();

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        })
    };

    let buffers = resolved
        .into_iter()
        .filter_map(|r| match r {
            Resolved::Buffer(buf) => Some(buf),
            Resolved::DynamicBuffer(buf, _) => Some(buf),
            _ => None,
        })
        .collect();

    (buffers, bind_group)
}
