use crate::{ecs::plugin::Plugin, graphics::backend::BackendPlugin};

pub mod backend;
pub mod frame;
pub mod targets;
pub mod window;

pub struct GraphicsPlugin;
impl Plugin for GraphicsPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.add_plugin(WindowPlugin).add_plugin(BackendPlugin)
    }
}

#[cfg(test)]
mod test {}
