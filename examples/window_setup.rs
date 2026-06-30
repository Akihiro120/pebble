use std::sync::Arc;

use pebble::prelude::*;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

struct WindowConfig {
    name: &'static str,
    width: u32,
    height: u32,
}

struct WindowResource {
    window: Arc<Window>,
}

struct WindowPlugin {
    config: WindowConfig,
}

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        let name = self.config.name;
        let width = self.config.width;
        let height = self.config.height;

        app.set_runner(move |mut app| {
            let event_loop = EventLoop::new().unwrap();
            let window = Arc::new(
                WindowBuilder::default()
                    .with_title(name)
                    .with_inner_size(PhysicalSize::new(width, height))
                    .build(&event_loop)
                    .unwrap(),
            );

            app.add_resource(WindowResource {
                window: window.clone(),
            });

            event_loop
                .run(move |event, elwt| match event {
                    Event::Resumed => {}
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            elwt.exit();
                        }
                        WindowEvent::RedrawRequested => {
                            app.update();
                        }
                        _ => {}
                    },
                    Event::AboutToWait => {
                        window.request_redraw();
                    }
                    _ => {}
                })
                .unwrap();
        });
    }
}

fn main() {
    App::new()
        .add_plugin(WindowPlugin {
            config: WindowConfig {
                name: "Window Setup Example",
                width: 1920,
                height: 1080,
            },
        })
        .build()
        .run();
}
