use crate::{
    plugin::Plugin,
    resources::Resources,
    system::{IntoSystem, System},
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    Render,
}

pub type AppRunner = Box<dyn FnMut(App)>;

pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: BTreeMap<SystemStage, Vec<Box<dyn System>>>,
    runner: Option<AppRunner>,
}

impl App {
    pub fn new() -> Self {
        let mut world = hecs::World::default();
        let resources = Resources::new(&mut world);

        Self {
            world: world,
            resources: resources,
            plugins: Vec::new(),
            systems: BTreeMap::new(),
            runner: Some(Box::new(|mut app| {
                loop {
                    app.update();
                }
            })),
        }
    }

    pub fn add_plugin(mut self, plugin: impl Plugin) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn add_resource(mut self, res: impl hecs::Component) -> Self {
        self.resources.insert_resource(&mut self.world, res);
        self
    }

    pub fn get_resource<'a, T: hecs::Component>(&'a self) -> hecs::Ref<'a, T> {
        self.resources.get_resource(&self.world)
    }

    pub fn get_resource_mut<'a, T: hecs::Component>(&'a self) -> hecs::RefMut<'a, T> {
        self.resources.get_resource_mut(&self.world)
    }

    pub fn add_system<Marker>(
        mut self,
        stage: SystemStage,
        system: impl IntoSystem<Marker> + 'static,
    ) -> Self {
        self.systems
            .entry(stage)
            .or_default()
            .push(Box::new(system.into_system()));
        self
    }

    pub fn build(mut self) -> Self {
        let plugins: Vec<_> = self.plugins.drain(..).collect();
        for plugin in plugins {
            plugin.build(&mut self);
        }

        if let Some(systems) = self.systems.remove(&SystemStage::Startup) {
            for mut system in systems {
                system.run(&self.world, &self.resources);
            }
            self.resources.get_command_buffer().run_on(&mut self.world);
        }
        self
    }

    pub fn update(&mut self) {
        for systems in self.systems.values_mut() {
            for system in systems {
                system.run(&self.world, &self.resources);
            }
        }
        self.resources.get_command_buffer().run_on(&mut self.world);
    }

    pub fn set_runner<F>(&mut self, runner: F) -> &mut Self
    where
        F: FnMut(App) + 'static,
    {
        self.runner = Some(Box::new(runner));
        self
    }

    pub fn run(mut self) {
        let mut runner = self.runner.take().unwrap();
        runner(self);
    }
}
