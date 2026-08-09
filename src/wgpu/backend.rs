use crate::{
    app::App,
    ecs::plugin::Plugin,
    rendering::{
        backend::{Backend, ColorTarget, FrameOperations, Pass},
        errors::AcquireError,
        sync::InitSender,
        window::{GPUSurfaceHandle, WindowConfig},
    },
    wgpu::{
        compute_pass::{CommandEncoder, ComputePass},
        render_pass::RenderPass,
        texture_format::TextureFormat,
        texture_view::TextureView,
        window::WinitWindow,
    },
};

/// The `wgpu`-backed [`Backend`] implementation. Inserted as a resource
/// once [`init`](Self::init) finishes (see the [`Backend`] trait docs for
/// how that's driven); everything in [`super`] that uploads to the GPU
/// (`Res<WGPUBackend>` in an [`Asset::upload`](crate::assets::upload::Asset::upload)
/// impl) reads `device`/`queue` directly off this.
///
/// `device`/`queue`/`surface`/`config` are `pub(crate)` — every builder in
/// [`wgpu::prelude`](super::prelude) that needs the device takes `&WGPUBackend`
/// directly instead of a raw `&wgpu::Device`. Surface dimensions/format are
/// exposed via [`surface_width`](Self::surface_width)/[`surface_height`](Self::surface_height)/
/// [`surface_format`](Self::surface_format) instead of the raw
/// `wgpu::SurfaceConfiguration`.
pub struct WGPUBackend {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    msaa_sample_count: u32,
    msaa_color: Option<wgpu::TextureView>,
}

impl WGPUBackend {
    /// Current surface width in pixels.
    pub fn surface_width(&self) -> u32 {
        self.config.width
    }

    /// Current surface height in pixels.
    pub fn surface_height(&self) -> u32 {
        self.config.height
    }

    /// The format the surface was negotiated at (a preferred sRGB format,
    /// chosen when the backend initializes).
    pub fn surface_format(&self) -> TextureFormat {
        self.config.format.into()
    }

    /// The multisample count [`ColorTarget::Default`]
    /// rendering (the window surface) currently uses — `1` (no MSAA) until
    /// [`set_msaa`](Self::set_msaa) is called. A material meant to render
    /// into the default target needs `Material { sample_count:
    /// backend.sample_count(), .. }` to match.
    pub fn sample_count(&self) -> u32 {
        self.msaa_sample_count
    }

    /// Turns on (`sample_count > 1`) or off (`sample_count: 1`) multisampled
    /// rendering into the window surface. Builds — or rebuilds, matching the
    /// current surface size — an internal multisampled color texture that
    /// [`ColorTarget::Default`]
    /// renders into and automatically resolves from into the real surface;
    /// [`resize`](Backend::resize) keeps it matched to the surface size
    /// afterward. Call this once at startup (or whenever you want to change
    /// the sample count), before building any material meant to render into
    /// the default target — see [`sample_count`](Self::sample_count). Any
    /// depth attachment used alongside it needs a matching
    /// [`RenderTargetTextureBuilder::sample_count`](super::texture_view::RenderTargetTextureBuilder::sample_count).
    pub fn set_msaa(&mut self, sample_count: u32) {
        self.msaa_sample_count = sample_count;
        self.rebuild_msaa_color();
    }

    fn rebuild_msaa_color(&mut self) {
        if self.msaa_sample_count <= 1 {
            self.msaa_color = None;
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pebble-msaa-color"),
            size: wgpu::Extent3d { width: self.config.width, height: self.config.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.msaa_sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.msaa_color = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
    }
}

impl WGPUBackend {
    async fn init_async(
        handle: impl GPUSurfaceHandle,
        width: u32,
        height: u32,
        sender: InitSender<Self>,
    ) {
        let backends = if cfg!(target_arch = "wasm32") {
            wgpu::Backends::BROWSER_WEBGPU
        } else {
            wgpu::Backends::PRIMARY
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            display: None,
            backends,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
        });

        let surface = instance.create_surface(handle).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (required_features, required_limits) = if cfg!(target_arch = "wasm32") {
            (wgpu::Features::empty(), wgpu::Limits::defaults())
        } else {
            (
                wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
                wgpu::Limits::default(),
            )
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                ..Default::default()
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        // Prefer Fifo (vsync) explicitly rather than trusting caps.present_modes[0] —
        // its ordering isn't guaranteed to put Fifo first, and an uncapped mode
        // (Immediate/Mailbox) here would tear and burn GPU cycles for no benefit.
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::PresentMode::Fifo)
            .unwrap_or(caps.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            width,
            height,
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        sender.send(WGPUBackend {
            device,
            queue,
            surface,
            config,
            msaa_sample_count: 1,
            msaa_color: None,
        });
    }
}

pub struct WGPUFrame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface_texture: wgpu::SurfaceTexture,
    /// Snapshot of `WGPUBackend::msaa_color` at acquire time — `Some` means
    /// `ColorTarget::Default` renders into this multisampled texture and
    /// resolves into `view` (the real surface) instead of rendering to
    /// `view` directly.
    msaa_view: Option<wgpu::TextureView>,
}

impl FrameOperations for WGPUFrame {
    type Context<'a> = RenderPass<'a>;
    type Attachment = TextureView;
    type DepthAttachment = TextureView;

    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_> {
        let color_attachments: Vec<_> = pass
            .colors
            .iter()
            .map(|target| {
                let (view, resolve_target, clear) = match target {
                    ColorTarget::Default { clear } => match &self.msaa_view {
                        Some(msaa) => (msaa, Some(&self.view), clear),
                        None => (&self.view, None, clear),
                    },
                    ColorTarget::Custom { attachment, clear } => (attachment.raw(), None, clear),
                };
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: clear
                            .map(|[r, g, b, a]| {
                                wgpu::LoadOp::Clear(wgpu::Color {
                                    r: r as f64,
                                    g: g as f64,
                                    b: b as f64,
                                    a: a as f64,
                                })
                            })
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();

        let depth_stencil_attachment =
            pass.depth
                .as_ref()
                .map(|d| wgpu::RenderPassDepthStencilAttachment {
                    view: d.attachment.raw(),
                    depth_ops: Some(wgpu::Operations {
                        load: d
                            .clear
                            .map(wgpu::LoadOp::Clear)
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                });

        let raw = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        RenderPass::new(raw)
    }
}

impl WGPUFrame {
    /// Begin a compute pass on this frame's command encoder.
    pub fn compute_pass(&mut self, label: Option<&str>) -> ComputePass<'_> {
        let raw = self.encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            timestamp_writes: None,
        });
        ComputePass::new(raw)
    }
}

impl WGPUBackend {
    /// Starts a command encoder for standalone GPU work not tied to an
    /// acquired frame — a compute dispatch outside `SystemStage::Render`,
    /// say. Begin a [`ComputePass`] on it via
    /// [`CommandEncoder::compute_pass`], then hand it to [`submit`](Self::submit)
    /// when done. Render passes don't need this —
    /// [`ActiveFrame::begin_pass`](crate::rendering::active_frame::ActiveFrame::begin_pass)
    /// manages its own frame-tied encoder internally.
    pub fn create_command_encoder(&self, label: Option<&str>) -> CommandEncoder {
        CommandEncoder::new(self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label }))
    }

    /// Finishes and submits `encoder`'s recorded commands to the queue.
    pub fn submit(&self, encoder: CommandEncoder) {
        self.queue.submit(std::iter::once(encoder.into_raw().finish()));
    }
}

impl Backend for WGPUBackend {
    type Frame = WGPUFrame;

    /// Native blocks the calling thread on [`init_async`](Self::init_async)
    /// via `pollster::block_on` (fine here — this only runs once, during
    /// [`App::build`](crate::app::App::build), and there's no other work
    /// competing for the thread yet). Web can't block its single thread, so
    /// it hands `init_async` to `wasm_bindgen_futures::spawn_local` instead
    /// and returns immediately — [`GraphicsPlugin`](crate::rendering::graphics_plugin::GraphicsPlugin)
    /// polls the resulting `sender`/[`InitReceiver`](crate::rendering::sync::InitReceiver)
    /// pair every tick either way, so callers don't need to know which path
    /// ran. A second [`Backend`] implementation should follow the same
    /// split if it also needs to run on both targets.
    fn init(handle: impl GPUSurfaceHandle, width: u32, height: u32, sender: InitSender<Self>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            pollster::block_on(Self::init_async(handle, width, height, sender));
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(Self::init_async(handle, width, height, sender));
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized — don't reconfigure to a degenerate size
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.rebuild_msaa_color();
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(AcquireError::Transient);
            }
            other => {
                return Err(AcquireError::Fatal(format!(
                    "unexpected surface state: {other:?}"
                )));
            }
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        Ok(WGPUFrame {
            encoder,
            view,
            surface_texture,
            msaa_view: self.msaa_color.clone(),
        })
    }

    fn present(&mut self, frame: Self::Frame) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.surface_texture.present();
    }
}

pub struct WGPUPlugin {
    config: WindowConfig,
}

impl WGPUPlugin {
    pub fn new(config: WindowConfig) -> Self {
        Self { config }
    }
}

impl Plugin for WGPUPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(crate::prelude::WindowPlugin::<WinitWindow>::new(
            WindowConfig {
                title: self.config.title.clone(),
                width: self.config.width,
                height: self.config.height,
            },
        ))
        .add_plugin(crate::wgpu::window::WindowControlPlugin)
        .add_plugin(crate::prelude::GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(crate::prelude::RenderPlugin::<WGPUBackend>::new())
        .add_plugin(crate::wgpu::textures::TexturePlugin)
        .add_plugin(crate::wgpu::texture_array::TextureArrayPlugin)
        .add_plugin(crate::wgpu::cubemap::CubemapPlugin)
        .add_plugin(crate::wgpu::mesh::MeshPlugin::new())
        .add_plugin(crate::wgpu::skinned_mesh::SkinnedMeshPlugin::new())
        .add_plugin(crate::wgpu::material::MaterialPlugin::new())
        .add_plugin(crate::wgpu::instance::MaterialInstancePlugin::new())
        .add_plugin(crate::wgpu::compute::ComputePlugin::new())
        .add_plugin(crate::wgpu::instance::ComputeInstancePlugin::new())
        .add_system(
            crate::app::SystemStage::Startup,
            crate::wgpu::samplers::init_global_samplers,
        )
        .add_resource(crate::wgpu::layout::GlobalLayoutPool::default());
    }
}
