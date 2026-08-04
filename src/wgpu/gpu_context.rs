//! Never public — [`GpuContext`] is what lets a wrapped resource ([`Buffer`](super::buffer::Buffer),
//! [`GPUTexture`](super::textures::GPUTexture), ...) act on itself
//! (`.write(...)`) without its owner needing to thread `&wgpu::Queue`
//! through every call site by hand. `wgpu::Device`/`wgpu::Queue` are already
//! cheap, `Arc`-backed handles internally, so caching a clone of each inside
//! every wrapped resource costs nothing beyond two pointer-sized fields.

#[derive(Clone)]
pub(crate) struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuContext {
    pub(crate) fn from_backend(backend: &super::backend::WGPUBackend) -> Self {
        Self { device: backend.device.clone(), queue: backend.queue.clone() }
    }

    /// For tests, which have `device`/`queue` directly (via
    /// [`test_util::try_device`](super::test_util::try_device)) rather than
    /// a full [`WGPUBackend`](super::backend::WGPUBackend).
    #[cfg(test)]
    pub(crate) fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self { device, queue }
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
