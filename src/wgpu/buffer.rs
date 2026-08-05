use crate::threading::SpawnableFuture;
use crate::wgpu::gpu_context::GpuContext;

/// A GPU buffer that can act on itself — built via
/// [`BufferBuilder`](super::buffers::BufferBuilder), never constructed
/// directly. Carries its own device/queue access internally, so writing to
/// it doesn't need a `&wgpu::Queue` threaded in from the caller.
///
/// Opaque by design: there's no way to reach the underlying `wgpu::Buffer`
/// from outside this crate. Binding one into a bind group goes through
/// [`BindGroupBuilder`](super::buffers::BindGroupBuilder), which accepts
/// `&Buffer` directly.
pub struct Buffer {
    pub(crate) raw: wgpu::Buffer,
    pub(crate) ctx: GpuContext,
}

impl Buffer {
    pub(crate) fn new(raw: wgpu::Buffer, ctx: GpuContext) -> Self {
        Self { raw, ctx }
    }

    /// Overwrites this buffer's contents with `data`, starting at offset 0.
    pub fn write(&self, data: &[u8]) {
        self.ctx.queue().write_buffer(&self.raw, 0, data);
    }

    /// Writes `data` into this buffer at a byte offset — for updating one
    /// element of a [`DynamicBuffer`] without touching the others. Prefer
    /// [`DynamicBuffer::write_element`], which computes the offset for you
    /// from the buffer's own stride.
    pub fn write_at(&self, offset: u64, data: &[u8]) {
        self.ctx.queue().write_buffer(&self.raw, offset, data);
    }

    /// Size in bytes.
    pub fn size(&self) -> u64 {
        self.raw.size()
    }

    /// Copies this buffer's current contents back to the CPU. The copy
    /// itself is submitted eagerly, right away — do not call mid-frame;
    /// call after presenting or outside of frame encoding. Only the *wait
    /// for the GPU to finish mapping it* is deferred into the returned
    /// future.
    ///
    /// This doesn't run itself — drive it with
    /// [`AsyncEventWriter::spawn`](crate::prelude::AsyncEventWriter::spawn) to
    /// get the result delivered as an event, or
    /// [`BackgroundTasks::spawn_async`](crate::threading::BackgroundTasks::spawn_async)
    /// directly if you'd rather hold onto a
    /// [`TaskHandle`](crate::threading::TaskHandle) and poll it yourself.
    pub fn read(&self) -> impl SpawnableFuture<Vec<u8>> {
        readback(self.ctx.device(), self.ctx.queue(), &self.raw)
    }

    /// Same as [`read`](Self::read) but the resolved bytes are cast to `T`.
    pub fn read_as<T: bytemuck::Pod + Send + 'static>(&self) -> impl SpawnableFuture<Vec<T>> {
        let bytes = self.read();
        async move {
            let bytes = bytes.await;
            bytemuck::cast_slice(&bytes).to_vec()
        }
    }

    pub(crate) fn raw(&self) -> &wgpu::Buffer {
        &self.raw
    }
}

/// Shared by [`Buffer::read`] and (in the future) anything else that needs a
/// GPU→CPU readback — split out so the async staging-buffer dance lives in
/// exactly one place.
pub(crate) fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &wgpu::Buffer,
) -> impl SpawnableFuture<Vec<u8>> {
    let size = src.size();
    let staging = crate::wgpu::buffers::BufferBuilder::new()
        .usage(crate::wgpu::flags::BufferUsages::COPY_DST | crate::wgpu::flags::BufferUsages::MAP_READ)
        .size(size)
        .build_raw(device);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
    let idx = queue.submit(std::iter::once(encoder.finish()));

    #[cfg(not(target_arch = "wasm32"))]
    let device = device.clone();

    async move {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            // Native backends need an explicit poll for a queued
            // map_async callback to ever fire — nothing else drives
            // that here, so this blocks whichever thread is driving the
            // future until the mapping lands. Fine: this is meant to
            // run via `BackgroundTasks::spawn_async`, which already
            // dedicates a worker thread to exactly this kind of wait.
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: None,
            });
            rx.recv().unwrap().unwrap();
            let data = staging.slice(..).get_mapped_range().to_vec();
            staging.unmap();
            data
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = idx;
            let mapped: std::sync::Arc<std::sync::Mutex<Option<Result<(), wgpu::BufferAsyncError>>>> =
                std::sync::Arc::new(std::sync::Mutex::new(None));
            let waker: std::sync::Arc<std::sync::Mutex<Option<std::task::Waker>>> =
                std::sync::Arc::new(std::sync::Mutex::new(None));

            let mapped_cb = mapped.clone();
            let waker_cb = waker.clone();
            staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                *mapped_cb.lock().unwrap() = Some(r);
                if let Some(w) = waker_cb.lock().unwrap().take() {
                    w.wake();
                }
            });

            std::future::poll_fn(move |cx| {
                if let Some(result) = mapped.lock().unwrap().take() {
                    result.unwrap();
                    let data = staging.slice(..).get_mapped_range().to_vec();
                    staging.unmap();
                    std::task::Poll::Ready(data)
                } else {
                    *waker.lock().unwrap() = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            })
            .await
        }
    }
}

/// A buffer sized to hold many dynamically-offset elements — built via
/// [`DynamicBufferBuilder`](super::buffers::DynamicBufferBuilder). Bundles
/// the per-element stride (for [`write_element`](Self::write_element)) and
/// the true (unpadded) element size (for
/// [`BindGroupBuilder::dynamic_buffer`](super::buffers::BindGroupBuilder::dynamic_buffer))
/// alongside the buffer itself, so neither can drift out of sync with what
/// the buffer was actually built with.
pub struct DynamicBuffer {
    pub(crate) buffer: Buffer,
    pub(crate) stride: u64,
    pub(crate) element_size: u64,
}

impl DynamicBuffer {
    pub(crate) fn new(buffer: Buffer, stride: u64, element_size: u64) -> Self {
        Self { buffer, stride, element_size }
    }

    /// Writes `data` (expected to be [`element_size`](Self::element_size)
    /// bytes) into the slot for element `index`, computing its byte offset
    /// from this buffer's own stride.
    pub fn write_element(&self, index: u64, data: &[u8]) {
        self.buffer.write_at(index * self.stride, data);
    }

    /// The byte size of one element, as originally given to
    /// [`DynamicBufferBuilder::uniform`](super::buffers::DynamicBufferBuilder::uniform)/[`storage`](super::buffers::DynamicBufferBuilder::storage).
    pub fn element_size(&self) -> u64 {
        self.element_size
    }

    /// The aligned per-element stride — pass `index as u32 * stride as u32`
    /// as the dynamic offset to `set_bind_group` at draw/dispatch time.
    pub fn stride(&self) -> u64 {
        self.stride
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::buffers::BufferBuilder;
    use crate::wgpu::flags::BufferUsages;
    use crate::wgpu::test_util::with_device;

    fn ctx(device: &wgpu::Device, queue: &wgpu::Queue) -> GpuContext {
        GpuContext::new(device.clone(), queue.clone())
    }

    #[test]
    fn write_and_write_at_do_not_panic() {
        with_device!(device, queue, {
            let buffer = Buffer::new(
                BufferBuilder::new().usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST).size(16).build_raw(&device),
                ctx(&device, &queue),
            );
            buffer.write(&[1u8, 2, 3, 4]);
            buffer.write_at(8, &[5u8, 6, 7, 8]);
            assert_eq!(buffer.size(), 16);
        });
    }

    #[test]
    fn dynamic_buffer_write_element_does_not_panic_and_reports_its_own_sizing() {
        with_device!(device, queue, {
            let element_size = 16u64;
            let count = 4u64;
            let (usage, stride) = (
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                crate::wgpu::buffers::dynamic_uniform_offset_stride_raw(&device, element_size),
            );
            let raw = BufferBuilder::new().usage(usage).size(stride * count).build_raw(&device);
            let dynamic = DynamicBuffer::new(Buffer::new(raw, ctx(&device, &queue)), stride, element_size);

            assert_eq!(dynamic.element_size(), element_size);
            assert_eq!(dynamic.stride(), stride);
            assert!(dynamic.stride() >= dynamic.element_size(), "stride is alignment-padded, never smaller than the element");

            // Writing the last element must not overrun the buffer — this
            // is exactly the case a wrong stride/size calculation would
            // panic on inside wgpu's validation.
            dynamic.write_element(count - 1, &vec![0u8; element_size as usize]);
        });
    }

}

