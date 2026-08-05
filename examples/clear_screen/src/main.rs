use pebble::prelude::*;
use pebble::wgpu::backend::{WGPUBackend, WGPUPlugin};

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Clear Screen".to_string(),
            width: 1920,
            height: 1080,
        }))
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let _pass = active.render_context([0.2, 0.3, 0.3, 1.0]);
    }
}
