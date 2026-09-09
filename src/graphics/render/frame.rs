use crate::graphics::{
    pipeline::{buffers::Buffer, textures::GPUTexture},
    render::{
        compute_pass::ComputePass, render_pass::RenderPass, targets::Pass,
        timestamps::GpuTimestamps,
    },
};

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
        self.begin_inner(pass, None)
    }

    /// [`Frame::begin`], bracketed by GPU timestamps under `label`. Untimed
    /// if the scope pool is full.
    pub fn begin_timed<'a>(
        &'a mut self,
        pass: Pass,
        label: &'static str,
        timestamps: &mut GpuTimestamps,
    ) -> RenderPass<'a> {
        let scope = timestamps.claim(label);
        self.begin_inner(pass, scope.map(|s| (s, timestamps.raw())))
    }

    fn begin_inner<'a>(
        &'a mut self,
        pass: Pass,
        scope: Option<((u32, u32), &wgpu::QuerySet)>,
    ) -> RenderPass<'a> {
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

        let timestamp_writes = scope.map(|((begin, end), query_set)| wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(end),
        });

        RenderPass::new(self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes,
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
        self.begin_compute_inner(label, None)
    }

    /// [`Frame::begin_compute`], bracketed by GPU timestamps under `label`.
    /// Untimed if the scope pool is full.
    pub fn begin_compute_timed<'a>(
        &'a mut self,
        label: &'static str,
        timestamps: &mut GpuTimestamps,
    ) -> ComputePass<'a> {
        let scope = timestamps.claim(label);
        self.begin_compute_inner(Some(label), scope.map(|s| (s, timestamps.raw())))
    }

    fn begin_compute_inner<'a>(
        &'a mut self,
        label: Option<&str>,
        scope: Option<((u32, u32), &wgpu::QuerySet)>,
    ) -> ComputePass<'a> {
        self.rotate_encoder();
        let timestamp_writes = scope.map(|((begin, end), query_set)| wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(end),
        });
        ComputePass::new(
            self.encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor { label, timestamp_writes }),
        )
    }

    /// Records a buffer-to-buffer copy into this frame's encoder, sequenced
    /// after whatever has already been recorded — so a copy issued after
    /// [`resolve_timestamps`](Self::resolve_timestamps) reads the resolved
    /// values, not the previous frame's.
    pub fn copy_buffer(&mut self, src: &Buffer, dst: &Buffer) {
        self.encoder
            .copy_buffer_to_buffer(src.raw(), 0, dst.raw(), 0, src.size());
    }

    /// Records a copy of `src`'s mip 0 into `dst`, for CPU readback, sequenced
    /// after whatever has already been recorded — so a copy issued after the
    /// passes that draw into `src` sees this frame's image, not the last one's.
    ///
    /// Returns the **row stride in bytes**. WebGPU requires each row of a
    /// texture-to-buffer copy to begin on a 256-byte boundary, so the stride is
    /// the texture's row size rounded up: `dst` must hold `stride * height`
    /// bytes, and the padding has to be dropped again on the way out. It is
    /// returned rather than hidden because the caller has to de-pad regardless,
    /// and a silently padded buffer is a subtle way to get a skewed image.
    ///
    /// `src` needs `TextureUsages::COPY_SRC` (see
    /// [`Texture::with_extra_usage`](crate::graphics::pipeline::textures::Texture::with_extra_usage));
    /// `dst` needs `COPY_DST`, plus `MAP_READ` to be read back.
    pub fn copy_texture_to_buffer(&mut self, src: &GPUTexture, dst: &Buffer) -> u32 {
        let texture = src.raw();
        let bytes_per_pixel = texture
            .format()
            .block_copy_size(None)
            .expect("a texture copied to a buffer must have a fixed block size");
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let stride = (src.width() * bytes_per_pixel).div_ceil(align) * align;

        self.encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: dst.raw(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(src.height()),
                },
            },
            wgpu::Extent3d {
                width: src.width(),
                height: src.height(),
                depth_or_array_layers: 1,
            },
        );
        stride
    }

    /// Resolves every scope claimed this frame into `dst`, which needs
    /// `BufferUsages::QUERY_RESOLVE` (plus `COPY_SRC` to be readable). Call
    /// once, after the last timed pass — the resolve gets its own command
    /// buffer, sequenced after the passes it reads.
    pub fn resolve_timestamps(&mut self, timestamps: &mut GpuTimestamps, dst: &Buffer) {
        let queries = timestamps.used() * 2;
        if queries == 0 {
            return;
        }
        timestamps.snapshot_resolved();
        self.rotate_encoder();
        self.encoder
            .resolve_query_set(timestamps.raw(), 0..queries, dst.raw(), 0);
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
