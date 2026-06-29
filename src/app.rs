use crate::{
    plugin::Plugin,
    system::{IntoSystem, System},
};

pub struct App {
    pub world: hecs::World,
    systems: Vec<Box<dyn System>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            world: hecs::World::default(),
            systems: Vec::new(),
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        plugin.build(self);
        self
    }

    pub fn add_resource(&mut self) -> &mut Self {
        self
    }

    pub fn add_system<Marker>(&mut self, system: impl IntoSystem<Marker>) -> &mut Self {
        self.systems.push(Box::new(system.into_system()));
        self
    }

    pub fn update(&mut self) {
        for system in self.systems.iter_mut() {
            system.run(&self.world);
        }
    }

    pub fn run<F>(&mut self, runner: F)
    where
        F: FnOnce(&mut App),
    {
        // for plugin in &self.plugins {
        //     plugin.build(&mut self);
        // }

        runner(self);
    }
}
