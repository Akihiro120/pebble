use crate::graphics::targets::Pass;

pub struct Frame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface: wgpu::SurfaceTexture
}

impl Frame {
    pub fn begin<'a>(&'a mut self, pass: Pass) -> wgpu::RenderPass<'a> {
        let color_attachments: Vec<_> = pass
            .colors
            .iter()
            .map(|target| {
                let view = target.attachment.unwrap_or(&self.view);

                Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            view: &d.attachment,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(d.clear.unwrap()),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None
        });

        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None
        })
    }
}

pub struct ActiveFrame<'a> {
    frame: &'a mut Frame
}

impl<'a> ActiveFrame<'a>{
    pub fn begin_pass(&'a mut self, pass: Pass) -> wgpu::RenderPass<'a> {
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

pub struct CurrentFrame {
    frame: Option<Frame>
}

impl CurrentFrame {
    pub fn active<'a>(&'a mut self) -> Option<ActiveFrame<'a>>{
        self.frame.as_mut().map(|f| ActiveFrame {
            frame: f
        })
    }
}
