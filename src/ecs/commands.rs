use crate::ecs::{
    resources::{Resources, Write},
    system_param::SystemParam,
};

#[derive(Default)]
pub struct ResourceCommandQueue(pub(crate) Vec<Box<dyn FnOnce(&mut Resources)>>);

pub struct Commands<'a> {
    buffer: Write<'a, hecs::CommandBuffer>,
    resource_commands: Write<'a, ResourceCommandQueue>,
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
    // queue a resource to be inserted once commands are synced
    pub fn insert_resource<T: 'static>(&mut self, value: T) {
        self.resource_commands
            .0
            .push(Box::new(move |resources| resources.insert(value)));
    }

    // queue a resource to be removed once commands are synced
    pub fn remove_resource<T: 'static>(&mut self) {
        self.resource_commands.0.push(Box::new(|resources| {
            resources.remove::<T>();
        }));
    }
}

impl SystemParam for Commands<'_> {
    type Item<'w> = Commands<'w>;
    type State = ((), ());

    fn fetch<'w>(
        world: &'w hecs::World,
        resources: &'w Resources,
        state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        Commands {
            buffer: Write::fetch(world, resources, &mut state.0),
            resource_commands: Write::fetch(world, resources, &mut state.1),
        }
    }
}
