use crate::ecs::{
    commands::{ResourceCommandQueue, TriggerQueue},
    resources::Resources,
    system_param::{IntoSystem, System},
};

#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System>>,
}

impl Schedule {
    pub fn add_system<Params: 'static>(
        &mut self,
        system: impl IntoSystem<Params> + 'static,
    ) -> &mut Self {
        self.systems.push(Box::new(system.into_system()));
        self
    }

    pub fn run(&mut self, world: &mut hecs::World, resources: &mut Resources) {
        for system in &mut self.systems {
            system.run(world, &*resources);
        }

        // sync entity commands
        resources.get_mut::<hecs::CommandBuffer>().run_on(world);

        // sync resource commands
        if resources.contains::<ResourceCommandQueue>() {
            let commands = std::mem::take(&mut resources.get_mut::<ResourceCommandQueue>().0);
            for command in commands {
                command(resources);
            }
        }

        // sync triggered observers
        if resources.contains::<TriggerQueue>() {
            let triggers = std::mem::take(&mut resources.get_mut::<TriggerQueue>().0);
            for trigger in triggers {
                trigger(world, resources);
            }
        }
    }
}
