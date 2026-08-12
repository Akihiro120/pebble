use crate::graphics::render::{render_pass::RenderPass, targets::Pass};

/// The acquired swapchain frame for this tick — access it via
/// [`CurrentFrame::active`], not directly.
pub struct Frame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface: wgpu::SurfaceTexture
}

impl Frame {
    pub(crate) fn new(encoder: wgpu::CommandEncoder, view: wgpu::TextureView, surface: wgpu::SurfaceTexture) -> Self {
        Self { encoder, view, surface }
    }

    pub(crate) fn finish(self) -> (wgpu::CommandEncoder, wgpu::SurfaceTexture) {
        (self.encoder, self.surface)
    }

    /// Begins a render pass for `pass`'s color/depth targets — an
    /// unattached color target falls back to the swapchain's own view.
    pub fn begin<'a>(&'a mut self, pass: Pass) -> RenderPass<'a> {
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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: target.clear[0] as f64,
                            g: target.clear[1] as f64,
                            b: target.clear[2] as f64,
                            a: target.clear[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store
                    }
                })
            }).collect();

        let depth_stencil_attachment = pass.depth.as_ref().map(|d| wgpu::RenderPassDepthStencilAttachment {
            view: d.attachment.raw(),
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(d.clear.unwrap()),
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
