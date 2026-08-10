use crate::ecs::{
    resources::{Resources, Write},
    system_param::SystemParam,
};

pub struct Commands<'a> {
    buffer: Write<'a, hecs::CommandBuffer>,
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

impl SystemParam for Commands<'_> {
    type Item<'w> = Commands<'w>;
    type State = ();

    fn fetch<'w>(
        world: &'w hecs::World,
        resources: &'w Resources,
        state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        Commands {
            buffer: Write::fetch(world, resources, state),
        }
    }
}
