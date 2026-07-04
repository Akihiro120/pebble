use std::sync::Arc;

use pebble::prelude::*;
use winit::{
    dpi::PhysicalSize,
    event::{
        Event::{self},
        WindowEvent,
    },
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

pub struct WinitWindow {
    pub window: Arc<Window>,
    pub event_loop: EventLoop<()>,
}

impl WindowProvider for WinitWindow {
    type Handle = Arc<winit::window::Window>;

    fn create(config: &WindowConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let window = Arc::new(
            WindowBuilder::new()
                .with_title(config.name)
                .with_inner_size(PhysicalSize::new(config.width, config.height))
                .build(&event_loop)
                .unwrap(),
        );

        Self { window, event_loop }
    }

    fn size(handle: &Self::Handle) -> (u32, u32) {
        let s = handle.inner_size();
        (s.width, s.height)
    }

    fn handle(&self) -> &Self::Handle {
        &self.window
    }
}

impl RenderTarget for WinitWindow {}
impl WindowRunner for WinitWindow {
    fn run(self, mut on_frame: impl FnMut() + 'static) {
        self.event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::RedrawRequested => on_frame(),
                    _ => {}
                },
                Event::AboutToWait => self.window.request_redraw(),
                _ => {}
            })
            .unwrap()
    }
}
