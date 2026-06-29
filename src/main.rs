use crate::{app::App, plugin::Plugin};

mod app;
mod commands;
mod plugin;
mod system;

struct HelloWorldPlugin;

impl Plugin for HelloWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(hello_world);
    }
}

fn hello_world() {
    println!("Hello, World! from the HelloWorldPlugin");
}

fn main() {
    App::new()
        .add_plugin(HelloWorldPlugin {})
        .run(run_application);
}

fn run_application(app: &mut App) {
    app.update();
}
