use std::sync::Arc;

use crate::{
    app::BackendReady,
    ecs::{
        commands::Commands,
        plugin::Plugin,
        resources::{Read, Write},
    },
};

pub struct Backend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_configuration: wgpu::SurfaceConfiguration,
}

pub(crate) struct BackendPlugin;
impl Plugin for BackendPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.add_system(crate::ecs::system::SystemStage::Update, obtain_gpu)
            .add_system(crate::ecs::system::SystemStage::Update, poll_gpu)
            .add_system(
                crate::ecs::system::SystemStage::Update,
                clean_up_gpu_acquisition_resources,
            )
    }
}

pub struct GPUReceiver {
    receiver: oneshot::Receiver<Backend>,
}

unsafe impl Send for GPUReceiver {}
unsafe impl Sync for GPUReceiver {}

async fn init_gpu(window: Arc<winit::window::Window>) -> Backend {
    let instance = wgpu::Instance::default();

    let surface = instance.create_surface(window.clone()).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap();

    let window_size = window.inner_size();

    let caps = surface.get_capabilities(&adapter);
    let surface_configuration = wgpu::SurfaceConfiguration {
        alpha_mode: caps.alpha_modes[0],
        present_mode: caps.present_modes[0],
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: caps.formats.iter().find(|f| !f.is_srgb()).unwrap().clone(),
        width: window_size.width,
        height: window_size.height,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_configuration);

    let backend = Backend {
        device,
        queue,
        surface,
        surface_configuration,
    };

    backend
}

pub(crate) fn obtain_gpu(mut commands: Commands) {
    // poll the window, if its available begin creation of gpu

    let (tx, rx) = oneshot::channel::<Backend>();

    let fut = async move {
        let result = init_gpu().await;
        let _ = tx.send(result);
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            pollster::block_on(fut);
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(fut);
    }

    commands.insert_resource(GPUReceiver { receiver: rx });
}

pub(crate) fn poll_gpu(
    mut commands: Commands,
    mut ready: Write<BackendReady>,
    receiver: Option<Read<GPUReceiver>>,
) {
    let Some(receiver) = receiver else {
        return;
    };

    if !ready.0 {
        // continue polling for the gpu
        match receiver.receiver.try_recv() {
            Ok(backend) => {
                println!("GPU Ready adding as resource");
                commands.insert_resource(backend);
                ready.0 = true;
            }
            Err(oneshot::TryRecvError::Empty) => return,
            Err(oneshot::TryRecvError::Disconnected) => {
                tracing::error!(
                    "GPU backend init sender was dropped without ever sending a value — the \
                         app has no usable backend and will stay idle forever"
                );
                return;
            }
        }
    }
}

pub(crate) fn clean_up_gpu_acquisition_resources() {}

pub(crate) fn begin_frame() {}

pub(crate) fn end_frame() {}
