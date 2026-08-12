use crate::graphics::render::Backend;

#[derive(Clone)]
pub(crate) struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuContext {
    pub(crate) fn from_backend(backend: &Backend) -> Self {
        Self { device: backend.device.clone(), queue: backend.queue.clone() }
    }

    pub(crate) fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub(crate) fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
