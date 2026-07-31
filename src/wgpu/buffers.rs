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

pub fn update_uniform_buffer(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &[u8]) {
    queue.write_buffer(buffer, 0, data);
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
