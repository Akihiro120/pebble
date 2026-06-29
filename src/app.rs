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

pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: BTreeMap<SystemStage, Vec<Box<dyn System>>>,
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
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn add_resource(&mut self, res: impl hecs::Component) -> &mut Self {
        self.resources.insert_resource(&mut self.world, res);
        self
    }

    pub fn add_system<Marker>(
        &mut self,
        stage: SystemStage,
        system: impl IntoSystem<Marker> + 'static,
    ) -> &mut Self {
        self.systems
            .entry(stage)
            .or_default()
            .push(Box::new(system.into_system()));
        self
    }

    pub fn build(&mut self) -> &mut App {
        let plugins: Vec<_> = self.plugins.drain(..).collect();
        for plugin in plugins {
            plugin.build(self);
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

    pub fn run<F>(&mut self, runner: F)
    where
        F: FnOnce(&mut App),
    {
        runner(self);
    }
}
