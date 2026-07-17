use crate::prelude::{Plugin, WindowConfig, WindowResource, WindowRunner};

/// Plugin that creates the platform window and installs it as a runner.
///
/// On build it:
/// 1. Creates the window via [`WindowProvider::create`](crate::rendering::window::WindowProvider::create).
/// 2. Inserts a [`WindowResource`] containing the handle and exposed value.
/// 3. Sets the app runner to `W::run`, which drives the frame loop.
pub struct WindowPlugin<W: WindowRunner> {
    pub config: WindowConfig,
    _marker: std::marker::PhantomData<W>,
}

impl<W> WindowPlugin<W>
where
    W: WindowRunner,
{
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<W> Plugin for WindowPlugin<W>
where
    W: WindowRunner,
    W::Handle: 'static + Send + Sync + Clone,
{
    fn build(&self, app: &mut crate::prelude::App) {
        let window_source = W::create(&self.config);
        let window_handle = window_source.handle().clone();
        let window_exposed = window_source.exposed().clone();

        app.add_resource(WindowResource::<W> {
            handle: window_handle,
            exposed: window_exposed,
        });
        app.set_runner(move |mut app| {
            window_source.run(move || app.update());
        });
    }
}
