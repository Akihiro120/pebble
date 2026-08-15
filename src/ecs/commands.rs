use crate::ecs::{
    observers::Observers,
    resources::{Resources, Write},
    system_param::SystemParam,
};

#[derive(Default)]
pub struct ResourceCommandQueue(pub(crate) Vec<Box<dyn FnOnce(&mut Resources)>>);

#[derive(Default)]
pub(crate) struct TriggerQueue(pub(crate) Vec<Box<dyn FnOnce(&hecs::World, &Resources)>>);

/// Deferred entity spawns and resource/observer mutations — applied at the
/// end of the current stage, not immediately. `Deref`s to `hecs::CommandBuffer`
/// for spawning/despawning entities and adding/removing components.
pub struct Commands<'a> {
    buffer: Write<'a, hecs::CommandBuffer>,
    resource_commands: Write<'a, ResourceCommandQueue>,
    triggers: Write<'a, TriggerQueue>,
}

impl<'a> std::ops::Deref for Commands<'a> {
    type Target = hecs::CommandBuffer;
    fn deref(&self) -> &hecs::CommandBuffer {
        &self.buffer
    }
}

impl<'a> std::ops::DerefMut for Commands<'a> {
    fn deref_mut(&mut self) -> &mut hecs::CommandBuffer {
        &mut self.buffer
    }
}

impl<'a> Commands<'a> {
    /// Queues a resource insert, applied once commands are synced.
    pub fn insert_resource<T: 'static>(&mut self, value: T) {
        self.resource_commands
            .0
            .push(Box::new(move |resources| resources.insert(value)));
    }

    /// Queues a resource removal, applied once commands are synced.
    pub fn remove_resource<T: 'static>(&mut self) {
        self.resource_commands.0.push(Box::new(|resources| {
            resources.remove::<T>();
        }));
    }

    /// Queues `event` to be dispatched to every observer registered for
    /// `E` via [`App::add_observer`](crate::app::App::add_observer), once
    /// commands are synced (right after the system that called this
    /// returns — same tick, not necessarily end of stage).
    pub fn trigger<E: 'static + Send + Sync>(&mut self, event: E) {
        self.triggers.0.push(Box::new(move |world, resources| {
            if !resources.contains::<Observers<E>>() {
                return;
            }
            let mut observers = std::mem::take(&mut resources.get_mut::<Observers<E>>().0);
            for observer in observers.iter_mut() {
                observer.run(&event, world, resources);
            }
            resources.get_mut::<Observers<E>>().0 = observers;
        }));
    }
}

impl SystemParam for Commands<'_> {
    type Item<'w> = Commands<'w>;
    type State = ((), (), ());

    fn fetch<'w>(
        world: &'w hecs::World,
        resources: &'w Resources,
        state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        Commands {
            buffer: Write::fetch(world, resources, &mut state.0),
            resource_commands: Write::fetch(world, resources, &mut state.1),
            triggers: Write::fetch(world, resources, &mut state.2),
        }
    }
}
