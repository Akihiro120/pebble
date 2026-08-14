use crate::{
    ecs::promise::Promise,
    graphics::{
        pipeline::{
            binding::BindGroupLayout, cubemap::GPUCubemap, samplers::Sampler,
            texture_array::GPUTextureArray, texture_view::TextureView, textures::GPUTexture,
        },
        render::{Backend, gpu_context::GpuContext},
        types::flags::BufferUsages,
    },
};

/// An opaque GPU buffer — built via [`BufferBuilder`]. No raw `wgpu` type
/// appears in its public API. Cheap to `Clone` — it's a handle to the same
/// underlying GPU buffer (`wgpu::Buffer` is `Arc`-backed), not a copy of
/// its contents.
#[derive(Clone)]
pub struct Buffer {
    pub(crate) raw: wgpu::Buffer,
    pub(crate) ctx: GpuContext,
}

impl Buffer {
    pub(crate) fn new(raw: wgpu::Buffer, ctx: GpuContext) -> Self {
        Self { raw, ctx }
    }

    /// Overwrites the buffer's contents from the start.
    pub fn write(&self, data: &[u8]) {
        self.ctx.queue().write_buffer(&self.raw, 0, data);
    }

    pub fn write_at(&self, offset: u64, data: &[u8]) {
        self.ctx.queue().write_buffer(&self.raw, offset, data);
    }

    /// Copies this buffer's contents back to the CPU. Requires
    /// `BufferUsages::COPY_SRC`. Poll the returned [`Promise`] each tick —
    /// it resolves once the GPU copy actually finishes.
    pub fn read(&self) -> Promise<Vec<u8>> {
        let usage = self.raw.usage();
        if !usage.contains(wgpu::BufferUsages::COPY_SRC) {
            panic!(
                "Buffer::read: buffer is missing BufferUsages::COPY_SRC — it can't be copied out of"
            );
        }

        let size = self.raw.size();
        let staging = self.ctx.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .ctx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(&self.raw, 0, &staging, 0, size);
        self.ctx.queue().submit(std::iter::once(encoder.finish()));

        let (fulfiller, promise) = Promise::new();
        // wgpu::Buffer is Arc-backed/Clone — this keeps the staging buffer
        // alive for the callback without borrowing `staging` itself
        let readback = staging.clone();
        staging.map_async(wgpu::MapMode::Read, .., move |result| {
            if result.is_ok() {
                let data = readback
                    .get_mapped_range(..)
                    .expect("Failed to get mapped range")
                    .to_vec();
                readback.unmap();
                fulfiller.fulfill(data);
            }
            // on error, `fulfiller` drops unfulfilled — Promise::poll
            // reports Disconnected
        });
        promise
    }

    pub fn size(&self) -> u64 {
        self.raw.size()
    }

    pub(crate) fn raw(&self) -> &wgpu::Buffer {
        &self.raw
    }
}

/// A buffer holding many fixed-size elements, each individually writable
/// and bindable at an aligned offset — for things like per-object uniform
/// data. Built via [`DynamicBufferBuilder`]. Cheap to `Clone`, same as
/// [`Buffer`].
#[derive(Clone)]
pub struct DynamicBuffer {
    pub(crate) buffer: Buffer,
    pub(crate) stride: u64,
    pub(crate) element_size: u64,
}

impl DynamicBuffer {
    pub(crate) fn new(buffer: Buffer, stride: u64, element_size: u64) -> Self {
        Self {
            buffer,
            stride,
            element_size,
        }
    }

    /// Overwrites element `index`'s data.
    pub fn write_element(&self, index: u64, data: &[u8]) {
        self.buffer.write_at(index * self.stride, data);
    }

    pub fn element_size(&self) -> u64 {
        self.element_size
    }

    pub fn stride(&self) -> u64 {
        self.stride
    }
}

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

/// Builds a [`Buffer`] — `empty(size)` or `with_data(bytes)`, then
/// `.with_usage(...)`/`.build(backend)`.
pub struct BufferBuilder<'a> {
    label: Option<&'a str>,
    usage: BufferUsages,
    contents: BufferContents<'a>,
}

impl<'a> BufferBuilder<'a> {
    pub fn empty(size: u64) -> Self {
        Self {
            label: None,
            usage: BufferUsages::empty(),
            contents: BufferContents::Empty(size),
        }
    }

    pub fn with_data(data: &'a [u8]) -> Self {
        Self {
            label: None,
            usage: BufferUsages::empty(),
            contents: BufferContents::Data(data),
        }
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_usage(mut self, usage: BufferUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_uniform(self) -> Self {
        self.with_usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST)
    }

    pub fn with_storage(self) -> Self {
        self.with_usage(BufferUsages::STORAGE | BufferUsages::COPY_DST)
    }

    pub fn build(self, backend: &Backend) -> Buffer {
        let device = &backend.device;
        check_buffer_size(device, self.label, self.usage, self.contents.size());
        let raw = match self.contents {
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
        };
        Buffer::new(raw, GpuContext::from_backend(backend))
    }
}

fn check_buffer_size(device: &wgpu::Device, label: Option<&str>, usage: BufferUsages, size: u64) {
    let limits = device.limits();
    let labeled = || label.map(|l| format!(" '{l}'")).unwrap_or_default();
    if size > limits.max_buffer_size {
        panic!(
            "buffer{}: {size} bytes exceeds this device's max_buffer_size ({})",
            labeled(),
            limits.max_buffer_size
        );
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

enum DynamicKind {
    Uniform,
    Storage,
}

/// Builds a [`DynamicBuffer`] — `uniform(element_size, count)`/`storage(...)`,
/// then `.build(backend)`. The actual per-element stride is rounded up to
/// the device's required offset alignment automatically.
pub struct DynamicBufferBuilder<'a> {
    label: Option<&'a str>,
    kind: DynamicKind,
    element_size: u64,
    count: u64,
}

impl<'a> DynamicBufferBuilder<'a> {
    pub fn uniform(element_size: u64, count: u64) -> Self {
        Self {
            label: None,
            kind: DynamicKind::Uniform,
            element_size,
            count,
        }
    }

    pub fn storage(element_size: u64, count: u64) -> Self {
        Self {
            label: None,
            kind: DynamicKind::Storage,
            element_size,
            count,
        }
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn build(self, backend: &Backend) -> DynamicBuffer {
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
        let buffer = BufferBuilder::empty(stride * self.count)
            .with_label(self.label)
            .with_usage(usage)
            .build(backend);
        DynamicBuffer::new(buffer, stride, self.element_size)
    }
}

pub fn dynamic_uniform_offset_stride(backend: &Backend, element_size: u64) -> u64 {
    align_to(
        element_size,
        backend.device.limits().min_uniform_buffer_offset_alignment as u64,
    )
}

pub fn dynamic_storage_offset_stride(backend: &Backend, element_size: u64) -> u64 {
    align_to(
        element_size,
        backend.device.limits().min_storage_buffer_offset_alignment as u64,
    )
}

fn align_to(size: u64, alignment: u64) -> u64 {
    size.div_ceil(alignment) * alignment
}

fn dynamic_buffer_binding(buffer: &wgpu::Buffer, element_size: u64) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset: 0,
        size: wgpu::BufferSize::new(element_size),
    })
}

/// An opaque bind group — built via [`BindGroupBuilder`].
pub struct BindGroup(wgpu::BindGroup);

impl BindGroup {
    pub(crate) fn raw(&self) -> &wgpu::BindGroup {
        &self.0
    }
}

/// Builds a [`BindGroup`] against a [`BindGroupLayout`] — `.with_buffer(...)`/
/// `.with_texture_2d(...)`/`.with_sampler(...)`/etc. in binding order (or
/// the `_at(binding, ...)` variant to place one explicitly), then
/// `.build(backend)`.
pub struct BindGroupBuilder<'a> {
    label: Option<&'a str>,
    layout: &'a wgpu::BindGroupLayout,
    entries: Vec<wgpu::BindGroupEntry<'a>>,
    next_binding: u32,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new(layout: &'a BindGroupLayout) -> Self {
        Self {
            label: None,
            layout: layout.raw(),
            entries: Vec::new(),
            next_binding: 0,
        }
    }

    pub fn with_label(mut self, label: impl Into<Option<&'a str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_buffer(self, buffer: &'a Buffer) -> Self {
        let binding = self.next_binding;
        self.with_buffer_at(binding, buffer)
    }

    pub fn with_buffer_at(mut self, binding: u32, buffer: &'a Buffer) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: buffer.raw().as_entire_binding(),
        });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn with_dynamic_buffer(self, buffer: &'a DynamicBuffer) -> Self {
        let binding = self.next_binding;
        self.with_dynamic_buffer_at(binding, buffer)
    }

    pub fn with_dynamic_buffer_at(mut self, binding: u32, buffer: &'a DynamicBuffer) -> Self {
        let resource = dynamic_buffer_binding(buffer.buffer.raw(), buffer.element_size);
        self.entries
            .push(wgpu::BindGroupEntry { binding, resource });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn with_texture_2d(self, texture: &'a GPUTexture) -> Self {
        let binding = self.next_binding;
        self.with_texture_2d_at(binding, texture)
    }

    pub fn with_texture_2d_at(self, binding: u32, texture: &'a GPUTexture) -> Self {
        self.texture_view_raw_at(binding, texture.view())
    }

    pub fn with_texture_array(self, texture: &'a GPUTextureArray) -> Self {
        let binding = self.next_binding;
        self.with_texture_array_at(binding, texture)
    }

    pub fn with_texture_array_at(self, binding: u32, texture: &'a GPUTextureArray) -> Self {
        self.texture_view_raw_at(binding, texture.view())
    }

    pub fn with_texture_cubemap(self, texture: &'a GPUCubemap) -> Self {
        let binding = self.next_binding;
        self.with_texture_cubemap_at(binding, texture)
    }

    pub fn with_texture_cubemap_at(self, binding: u32, texture: &'a GPUCubemap) -> Self {
        self.texture_view_raw_at(binding, texture.view())
    }

    pub fn with_texture_view(self, view: &'a TextureView) -> Self {
        let binding = self.next_binding;
        self.with_texture_view_at(binding, view)
    }

    pub fn with_texture_view_at(self, binding: u32, view: &'a TextureView) -> Self {
        self.texture_view_raw_at(binding, view.raw())
    }

    pub(crate) fn texture_view_raw_at(mut self, binding: u32, view: &'a wgpu::TextureView) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn with_sampler(self, sampler: &'a Sampler) -> Self {
        let binding = self.next_binding;
        self.with_sampler_at(binding, sampler)
    }

    pub fn with_sampler_at(mut self, binding: u32, sampler: &'a Sampler) -> Self {
        self.entries.push(wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Sampler(sampler.raw()),
        });
        self.next_binding = self.next_binding.max(binding + 1);
        self
    }

    pub fn build(self, backend: &Backend) -> BindGroup {
        BindGroup(
            backend
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: self.label,
                    layout: self.layout,
                    entries: &self.entries,
                }),
        )
    }
}
