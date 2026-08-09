use crate::ecs::{
    resources::Resources,
    system_param::{IntoSystem, StoredSystem},
};

#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn StoredSystem>>,
}

impl Schedule {
    pub fn add_system<Params: 'static>(&mut self, system: impl IntoSystem<Params>) -> &mut Self {
        self.systems.push(system.into_system());
        self
    }

    pub fn run(&mut self, world: &mut hecs::World, resources: &Resources) {
        for system in &mut self.systems {
            system.run(world, resources);
        }

        // sync
        resources.get_mut::<hecs::CommandBuffer>().run_on(world);
    }
}
