use crate::{
    plugin::Plugin,
    resources::Resources,
    system::{IntoSystem, System},
};

pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: Vec<Box<dyn System>>,
}

impl App {
    pub fn new() -> Self {
        let mut world = hecs::World::default();
        let resources = Resources::new(&mut world);

        Self {
            world: world,
            resources: resources,
            plugins: Vec::new(),
            systems: Vec::new(),
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

    pub fn add_system<Marker>(&mut self, system: impl IntoSystem<Marker> + 'static) -> &mut Self {
        self.systems.push(Box::new(system.into_system()));
        self
    }

    pub fn build(&mut self) -> &mut App {
        while let Some(plugin) = self.plugins.pop() {
            plugin.build(self);
        }
        self
    }

    pub fn update(&mut self) {
        for system in self.systems.iter_mut() {
            system.run(&self.world, &self.resources);
        }
    }

    pub fn run<F>(&mut self, runner: F)
    where
        F: FnOnce(&mut App),
    {
        runner(self);
    }
}
