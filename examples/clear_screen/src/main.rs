use examples_common::*;
use pebble::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();
    App::new()
        .add_plugin(WindowPlugin::<WinitWindow>::new(WindowConfig {
            title: "Clear Screen",
            width: 1920,
            height: 1080,
        }))
        .add_plugin(GraphicsPlugin::<WGPUBackend, WinitWindow>::new())
        .add_plugin(RenderPlugin::<WGPUBackend>::new())
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let _pass = active.render_context([0.2, 0.3, 0.3, 1.0]);
    }
}
