use crate::ecs::{
    plugin::Plugin, resources::Resources, schedule::Schedule, system_param::IntoSystem,
};

#[derive(Default)]
pub struct AppExit(pub bool);

pub struct App {
    world: hecs::World,
    resources: Resources,
    schedule: Schedule,
}

impl Default for App {
    fn default() -> Self {
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(AppExit::default());

        Self {
            world: hecs::World::default(),
            schedule: Schedule::default(),
            resources,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    // add a resource
    pub fn insert_resource<T: 'static>(mut self, resource: T) -> Self {
        self.resources.insert(resource);
        self
    }

    // remove a resource
    pub fn remove_resource<T: 'static>(mut self) -> Self {
        self.resources.remove::<T>();
        self
    }

    // add a plugin
    pub fn add_plugin<P: Plugin>(self, plugin: P) -> Self {
        plugin.build(self)
    }

    // add a system
    pub fn add_system<Params: 'static>(mut self, system: impl IntoSystem<Params>) -> Self {
        self.schedule.add_system(system);
        self
    }

    pub fn run(mut self) {
        loop {
            self.schedule.run(&mut self.world, &self.resources);
            if self.resources.get::<AppExit>().0 {
                break;
            }
        }
    }
}

pub struct AppBuilder {}

impl Default for AppBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl AppBuilder {
    // enable logging
    pub fn with_logging(self) -> Self {
        tracing_subscriber::fmt().init();
        self
    }

    // build the app
    pub fn build(&self) -> Option<App> {
        Some(App::default())
    }
}
