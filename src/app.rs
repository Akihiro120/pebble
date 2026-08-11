use std::collections::BTreeMap;

use crate::ecs::{
    commands::ResourceCommandQueue, plugin::Plugin, resources::Resources, schedule::Schedule,
    system::SystemStage, system_param::IntoSystem,
};

#[derive(Default)]
pub struct AppExit(pub bool);

#[derive(Default)]
pub struct BackendReady(pub bool);

pub struct App {
    world: hecs::World,
    resources: Resources,
    schedules: BTreeMap<SystemStage, Schedule>,
    pub(crate) gpu_schedules: BTreeMap<SystemStage, Schedule>,
}

impl Default for App {
    fn default() -> Self {
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(ResourceCommandQueue::default());
        resources.insert(AppExit::default());
        resources.insert(BackendReady::default());

        Self {
            world: hecs::World::default(),
            schedules: BTreeMap::new(),
            gpu_schedules: BTreeMap::new(),
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
    pub fn add_system<Params: 'static>(
        mut self,
        stage: SystemStage,
        system: impl IntoSystem<Params> + 'static,
    ) -> Self {
        self.schedules
            .entry(stage)
            .or_insert_with(Schedule::default)
            .add_system(system);
        self
    }

    pub(crate) fn add_gpu_system<Params: 'static>(
        mut self,
        stage: SystemStage,
        system: impl IntoSystem<Params> + 'static,
    ) -> Self {
        self.gpu_schedules
            .entry(stage)
            .or_insert_with(Schedule::default)
            .add_system(system);
        self
    }

    pub fn run(mut self) {
        // startup schedules run exactly once, before the main loop
        if let Some(mut startup) = self.schedules.remove(&SystemStage::Startup) {
            startup.run(&mut self.world, &mut self.resources);
        }

        loop {
            // keep attempting to obtain the gpu until its ready
            if !self.resources.get::<BackendReady>().0 {
                println!("Backend Pending");
                for (_, schedule) in self.gpu_schedules.iter_mut() {
                    schedule.run(&mut self.world, &mut self.resources);
                }
                std::thread::sleep(std::time::Duration::from_millis(16));
                continue;
            }

            // run the systems in stage order, only once the gpu backend is ready
            println!("Backend Obtained, Running Systems");
            for (_, schedule) in self.schedules.iter_mut() {
                schedule.run(&mut self.world, &mut self.resources);
            }
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
