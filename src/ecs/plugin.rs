use crate::app::App;

/// A composable unit of app setup — insert resources, register systems, or
/// add further plugins. Also implemented for any `FnOnce(App) -> App`, so a
/// plain closure works as a plugin without a named type.
pub trait Plugin {
    fn build(self, app: App) -> App;
}

impl<F> Plugin for F
where
    F: FnOnce(App) -> App,
{
    fn build(self, app: App) -> App {
        self(app)
    }
}
