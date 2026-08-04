use crate::{
    app::App,
    ecs::plugin::Plugin,
    rendering::{
        backend::{Backend, ColorTarget, FrameOperations, Pass},
        errors::AcquireError,
        sync::InitSender,
        window::{GPUSurfaceHandle, WindowConfig},
    },
    threading::SpawnableFuture,
    wgpu::window::WinitWindow,
};

/// The `wgpu`-backed [`Backend`] implementation. Inserted as a resource
/// once [`init`](Self::init) finishes (see the [`Backend`] trait docs for
/// how that's driven); everything in [`super`] that uploads to the GPU
/// (`Res<WGPUBackend>` in an [`Asset::upload`](crate::assets::upload::Asset::upload)
/// impl) reads `device`/`queue` directly off this.
///
/// `device`/`queue` are `pub` because some operations genuinely need them
/// (submitting command encoders, `queue.write_buffer`, resource types
/// [`wgpu::prelude`](super::prelude) doesn't cover) — but for building a
/// buffer, bind group layout, or bind group, reach for
/// [`wgpu::prelude`](super::prelude) first rather than hand-writing a
/// `wgpu::BufferDescriptor`/`BindGroupLayoutDescriptor`/`BindGroupDescriptor`
/// against `device` directly.
pub struct WGPUBackend {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
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
        });
    }
}

pub struct WGPUFrame {
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    surface_texture: wgpu::SurfaceTexture,
}

impl FrameOperations for WGPUFrame {
    type Context<'a> = wgpu::RenderPass<'a>;
    type Attachment = wgpu::TextureView;
    type DepthAttachment = wgpu::TextureView;

    fn begin(&mut self, pass: Pass<'_, Self>) -> Self::Context<'_> {
        let color_attachments: Vec<_> = pass
            .colors
            .iter()
            .map(|target| {
                let (view, clear) = match target {
                    ColorTarget::Default { clear } => (&self.view, clear),
                    ColorTarget::Custom { attachment, clear } => (*attachment, clear),
                };
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
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
                    view: d.attachment,
                    depth_ops: Some(wgpu::Operations {
                        load: d
                            .clear
                            .map(wgpu::LoadOp::Clear)
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                });

        self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

impl WGPUFrame {
    /// Begin a compute pass on this frame's command encoder.
    pub fn compute_pass(&mut self, label: Option<&str>) -> wgpu::ComputePass<'_> {
        self.encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label,
                timestamp_writes: None,
            })
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
    }

    fn acquire(&mut self) -> Result<Self::Frame, AcquireError> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Outdated => {
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
        })
    }

    fn present(&mut self, frame: Self::Frame) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.surface_texture.present();
    }
}

impl WGPUBackend {
    /// Copies `src` into a temporary staging buffer, begins a GPU readback,
    /// and returns a future that resolves to the copied bytes once it's
    /// done.
    ///
    /// The copy is submitted eagerly, right away — do not call mid-frame;
    /// call after `present` or outside of frame encoding. Only the *wait
    /// for the GPU to finish mapping it* is deferred into the returned
    /// future.
    ///
    /// This doesn't run itself — drive it with
    /// [`AsyncEventWriter::spawn`](crate::prelude::AsyncEventWriter::spawn) to get the
    /// result delivered as an event, or
    /// [`BackgroundTasks::spawn_async`](crate::threading::BackgroundTasks::spawn_async)
    /// directly if you'd rather hold onto a
    /// [`TaskHandle`](crate::threading::TaskHandle) and poll it yourself.
    pub fn readback_buffer(&self, src: &wgpu::Buffer) -> impl SpawnableFuture<Vec<u8>> {
        use crate::wgpu::buffers::BufferBuilder;

        let size = src.size();
        let staging = BufferBuilder::new()
            .usage(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ)
            .size(size)
            .build(&self.device);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        let idx = self.queue.submit(std::iter::once(encoder.finish()));

        #[cfg(not(target_arch = "wasm32"))]
        let device = self.device.clone();

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
                let mapped: std::sync::Arc<
                    std::sync::Mutex<Option<Result<(), wgpu::BufferAsyncError>>>,
                > = std::sync::Arc::new(std::sync::Mutex::new(None));
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

                std::future::poll_fn(|cx| {
                    let mut guard = mapped.lock().unwrap();
                    if let Some(r) = guard.take() {
                        std::task::Poll::Ready(r)
                    } else {
                        *waker.lock().unwrap() = Some(cx.waker().clone());
                        std::task::Poll::Pending
                    }
                })
                .await
                .unwrap();

                let data = staging.slice(..).get_mapped_range().to_vec();
                staging.unmap();
                data
            }
        }
    }

    /// Same as [`readback_buffer`](Self::readback_buffer) but the resolved
    /// bytes are cast to `T`.
    pub fn readback_buffer_as<T: bytemuck::Pod + Send + 'static>(
        &self,
        src: &wgpu::Buffer,
    ) -> impl SpawnableFuture<Vec<T>> {
        let bytes = self.readback_buffer(src);
        async move {
            let bytes = bytes.await;
            bytemuck::cast_slice(&bytes).to_vec()
        }
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
        .add_plugin(crate::prelude::GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(crate::prelude::RenderPlugin::<WGPUBackend>::new())
        .add_plugin(crate::wgpu::textures::TexturePlugin)
        .add_plugin(crate::wgpu::texture_array::TextureArrayPlugin)
        .add_plugin(crate::wgpu::cubemap::CubemapPlugin)
        .add_plugin(crate::wgpu::mesh::MeshPlugin::new())
        .add_plugin(crate::wgpu::material::MaterialPlugin::new())
        .add_plugin(crate::wgpu::instance::MaterialInstancePlugin::new())
        .add_plugin(crate::wgpu::compute::ComputePlugin::new())
        .add_plugin(crate::wgpu::instance::ComputeInstancePlugin::new())
        .add_plugin(crate::prelude::LazyResourcePlugin::<
            WGPUBackend,
            crate::wgpu::samplers::GlobalSamplers,
        >::new());
    }
}
