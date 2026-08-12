use crate::ecs::{
    commands::{ResourceCommandQueue, TriggerQueue},
    resources::Resources,
    system_param::{IntoSystem, System},
};

/// An ordered list of systems, run together — one `Schedule` backs each
/// [`SystemStage`](crate::ecs::system::SystemStage). Running it also
/// flushes any deferred `Commands` from the systems that just ran.
#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System>>,
}

impl Schedule {
    /// Appends `system` to the end of this schedule.
    pub fn add_system<Params: 'static>(
        &mut self,
        system: impl IntoSystem<Params> + 'static,
    ) -> &mut Self {
        self.systems.push(Box::new(system.into_system()));
        self
    }

    /// Runs every system in order, then flushes deferred entity spawns,
    /// resource commands, and triggered observers from this run.
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
