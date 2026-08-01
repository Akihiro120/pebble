pub enum BufferSource<'a> {
    Data(&'a [u8]),
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

pub enum BindingResource<'a> {
    UniformBuffer(BufferSource<'a>),
    StorageBuffer(BufferSource<'a>),
    TextureView(&'a wgpu::TextureView),
    Sampler(&'a wgpu::Sampler),
}

pub fn build_buffer(device: &wgpu::Device, contents: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents,
        usage,
    })
}

pub fn build_buffer_sized(device: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage,
        mapped_at_creation: false,
    })
}

pub fn resolve_buffer(device: &wgpu::Device, source: BufferSource<'_>, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    match source {
        BufferSource::Data(data) => build_buffer(device, data, usage),
        BufferSource::Buffer(buffer) => buffer,
    }
}

pub fn resolve_uniform_buffer(device: &wgpu::Device, source: BufferSource<'_>) -> wgpu::Buffer {
    resolve_buffer(device, source, wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST)
}

pub fn resolve_storage_buffer(device: &wgpu::Device, source: BufferSource<'_>) -> wgpu::Buffer {
    resolve_buffer(device, source, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
}

pub fn build_uniform_bind_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: impl Into<BufferSource<'a>>,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let (mut buffers, bind_group) = build_bind_group(device, layout, vec![BindingResource::UniformBuffer(source.into())]);
    (buffers.remove(0), bind_group)
}

pub fn build_storage_bind_group<'a>(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: impl Into<BufferSource<'a>>,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let (mut buffers, bind_group) = build_bind_group(device, layout, vec![BindingResource::StorageBuffer(source.into())]);
    (buffers.remove(0), bind_group)
}

pub fn update_uniform_buffer(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &[u8]) {
    queue.write_buffer(buffer, 0, data);
}

/// Writes `data` into `buffer` at a byte offset, for updating one element of a
/// dynamically-offset buffer without touching the others. `offset` should be a
/// multiple of the stride returned by [`dynamic_offset_stride`].
pub fn update_buffer_at(queue: &wgpu::Queue, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
    queue.write_buffer(buffer, offset, data);
}

/// Rounds `element_size` up to the device's required alignment for dynamic offsets on
/// uniform buffers, giving the stride to use when packing multiple elements into one
/// buffer for use with [`MaterialBindingKind::dynamic_uniform_buffer`](super::material::MaterialBindingKind::dynamic_uniform_buffer).
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
        TextureView(&'a wgpu::TextureView),
        Sampler(&'a wgpu::Sampler),
    }

    let resolved: Vec<Resolved> = resources
        .into_iter()
        .map(|r| match r {
            BindingResource::UniformBuffer(src) => Resolved::Buffer(resolve_uniform_buffer(device, src)),
            BindingResource::StorageBuffer(src) => Resolved::Buffer(resolve_storage_buffer(device, src)),
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
            _ => None,
        })
        .collect();

    (buffers, bind_group)
}
