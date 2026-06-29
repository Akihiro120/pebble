use crate::{
    app::App,
    plugin::Plugin,
    system::{Res, ResMut},
};

mod app;
mod commands;
mod plugin;
mod resources;
mod system;

#[derive(Default)]
struct Time {
    pub time: f32,
}

struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Time::default())
            .add_system(update_time)
            .add_system(display_time);
    }
}

fn update_time(mut time: ResMut<Time>) {
    time.time += 1.0;
}

fn display_time(time: Res<Time>) {
    println!("{}", time.time);
}

fn main() {
    App::new()
        .add_plugin(TimePlugin {})
        .build()
        .run(run_application);
}

fn run_application(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}
