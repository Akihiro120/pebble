use crate::ecs::{
    resources::{ResMut, Resources},
    system_param::SystemParam,
};

pub struct Commands<'a> {
    buffer: ResMut<'a, hecs::CommandBuffer>,
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
    fn fetch<'w>(world: &'w hecs::World, resources: &'w Resources) -> Self::Item<'w> {
        Commands {
            buffer: ResMut::fetch(world, resources),
        }
    }
}

pub trait SystemParamFunction<Params>: 'static {
    fn run_system(&mut self, world: &hecs::World, resources: &Resources);
}
