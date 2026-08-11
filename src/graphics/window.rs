use crate::ecs::plugin::Plugin;

pub struct WindowPlugin;
impl Plugin for WindowPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app
    }
}
