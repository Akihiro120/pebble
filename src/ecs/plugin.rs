use crate::app::App;

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
