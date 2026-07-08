use crate::prelude::{Plugin, WindowConfig, WindowResource, WindowRunner};

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

        app.add_resource(WindowResource::<W> {
            handle: window_handle,
        });
        app.set_runner(move |mut app| {
            window_source.run(move || app.update());
        });
    }
}
