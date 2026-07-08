use std::cell::{RefCell, RefMut};

pub struct Resources {
    pub(crate) resource_entity: hecs::Entity,
    cmds: RefCell<hecs::CommandBuffer>,
}

impl Resources {
    pub fn new(world: &mut hecs::World) -> Self {
        Self {
            resource_entity: world.spawn(()),
            cmds: RefCell::new(hecs::CommandBuffer::default()),
        }
    }

    pub fn insert_resource<T>(&mut self, world: &mut hecs::World, res: T)
    where
        T: hecs::Component,
    {
        world.insert_one(self.resource_entity, res).ok();
    }

    pub fn get_resource<'a, T>(&self, world: &'a hecs::World) -> hecs::Ref<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&T>(self.resource_entity)
            .unwrap_or_else(|_| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    pub fn get_resource_mut<'a, T>(&self, world: &'a hecs::World) -> hecs::RefMut<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&mut T>(self.resource_entity)
            .unwrap_or_else(|_| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    pub fn has_resource<T>(&self, world: &hecs::World) -> bool
    where
        T: hecs::Component,
    {
        if let Ok(_) = world.get::<&T>(self.resource_entity) {
            return true;
        }

        false
    }

    pub fn get_command_buffer<'a>(&'a self) -> RefMut<'a, hecs::CommandBuffer> {
        self.cmds.borrow_mut()
    }

    pub fn try_insert<T>(&mut self, world: &mut hecs::World, res: T) -> bool
    where
        T: hecs::Component,
    {
        if self.has_resource::<T>(world) {
            false
        } else {
            world.insert_one(self.resource_entity, res).ok();
            true
        }
    }
}
