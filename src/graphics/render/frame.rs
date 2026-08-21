use crate::graphics::render::{compute_pass::ComputePass, render_pass::RenderPass, targets::Pass};

/// The acquired swapchain frame for this tick — access it via
/// [`CurrentFrame::active`], not directly.
///
/// Owns the frame's encoder lifecycle: every pass records into its own
/// command encoder. iOS 18 WebKit invalidates an encoder once a pass ends,
/// so reuse across passes fails there ("encoder state is not valid") — and
/// nothing is gained by relying on it. Ordering is unaffected: the sealed
/// buffers are submitted together, in recording order, by `end_frame`.
pub struct Frame {
    device: wgpu::Device,
    encoder: wgpu::CommandEncoder,
    encoder_used: bool,
    recorded: Vec<wgpu::CommandBuffer>,
    view: wgpu::TextureView,
    surface: wgpu::SurfaceTexture
}

impl Frame {
    pub(crate) fn new(device: wgpu::Device, view: wgpu::TextureView, surface: wgpu::SurfaceTexture) -> Self {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        Self { device, encoder, encoder_used: false, recorded: Vec::new(), view, surface }
    }

    pub(crate) fn finish(mut self) -> (Vec<wgpu::CommandBuffer>, wgpu::SurfaceTexture) {
        self.recorded.push(self.encoder.finish());
        (self.recorded, self.surface)
    }

    /// Called at the start of every pass: seals the encoder holding the
    /// previous pass, if any, and puts a fresh one in its place.
    fn rotate_encoder(&mut self) {
        if self.encoder_used {
            let sealed = std::mem::replace(
                &mut self.encoder,
                self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default()),
            );
            self.recorded.push(sealed.finish());
        }
        self.encoder_used = true;
    }

    /// Begins a render pass for `pass`'s color/depth targets — an
    /// unattached color target falls back to the swapchain's own view.
    pub fn begin<'a>(&'a mut self, pass: Pass) -> RenderPass<'a> {
        self.rotate_encoder();
        let color_attachments: Vec<_> = pass
            .colors
            .iter()
            .map(|target| {
                let view = target.attachment.map(|t| t.raw()).unwrap_or(&self.view);

                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: target
                            .clear
                            .map(|[r, g, b, a]| {
                                wgpu::LoadOp::Clear(wgpu::Color {
                                    r: r as f64,
                                    g: g as f64,
                                    b: b as f64,
                                    a: a as f64,
                                })
                            })
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store
                    }
                })
            }).collect();

        let depth_stencil_attachment = pass.depth.as_ref().map(|d| wgpu::RenderPassDepthStencilAttachment {
            view: d.attachment.raw(),
            depth_ops: Some(wgpu::Operations {
                load: d.clear.map(wgpu::LoadOp::Clear).unwrap_or(wgpu::LoadOp::Load),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None
        });

        RenderPass::new(self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None
        }))
    }

    /// Begins a compute pass on this frame's command encoder, so compute
    /// work is recorded and submitted together with the frame's render
    /// passes and keeps its ordering relative to them.
    ///
    /// [`Backend::dispatch_compute`](crate::graphics::render::Backend::dispatch_compute)
    /// remains the right call for compute that doesn't need a frame — it
    /// records into its own encoder and submits immediately.
    pub fn begin_compute<'a>(&'a mut self, label: Option<&str>) -> ComputePass<'a> {
        self.rotate_encoder();
        ComputePass::new(
            self.encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor { label, timestamp_writes: None }),
        )
    }
}

/// A frame known to be acquired, from [`CurrentFrame::active`]. `Deref`s to
/// [`Frame`].
pub struct ActiveFrame<'a> {
    frame: &'a mut Frame
}

impl<'a> ActiveFrame<'a>{
    /// Begins a render pass — see [`Frame::begin`].
    pub fn begin_pass(&'a mut self, pass: Pass) -> RenderPass<'a> {
        self.frame.begin(pass)
    }
}

impl<'a> std::ops::Deref for ActiveFrame<'a> {
    type Target = Frame;
    
    fn deref(&self) -> &Self::Target {
        self.frame
    }
}

impl<'a> std::ops::DerefMut for ActiveFrame<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.frame
    }
}

/// Resource holding this tick's acquired frame, if any — `None` when the
/// surface couldn't be acquired (occluded, resizing, etc.), in which case
/// rendering this tick should just be skipped.
#[derive(Default)]
pub struct CurrentFrame {
    frame: Option<Frame>
}

impl CurrentFrame {
    pub(crate) fn set(&mut self, frame: Frame) {
        self.frame = Some(frame);
    }

    pub(crate) fn take(&mut self) -> Option<Frame> {
        self.frame.take()
    }

    /// The active frame to render into, if one was acquired this tick.
    pub fn active<'a>(&'a mut self) -> Option<ActiveFrame<'a>>{
        self.frame.as_mut().map(|f| ActiveFrame {
            frame: f
        })
    }
}
