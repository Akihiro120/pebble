use std::cell::{RefCell, RefMut};

pub struct Resources {
    pub(crate) res_id: hecs::Entity,
    cmds: RefCell<hecs::CommandBuffer>,
}

impl Resources {
    pub fn new(world: &mut hecs::World) -> Self {
        Self {
            res_id: world.spawn(()),
            cmds: RefCell::new(hecs::CommandBuffer::default()),
        }
    }

    pub fn insert_resource<T>(&mut self, world: &mut hecs::World, res: T)
    where
        T: hecs::Component,
    {
        world.insert_one(self.res_id, res).ok();
    }

    pub fn get_resource<'a, T>(&self, world: &'a hecs::World) -> hecs::Ref<'a, T>
    where
        T: hecs::Component,
    {
        world.get::<&T>(self.res_id).expect("resource not found")
    }

    pub fn has_resource<T>(&self, world: &hecs::World) -> bool
    where
        T: hecs::Component,
    {
        if let Ok(_) = world.get::<&T>(self.res_id) {
            return true;
        }

        false
    }

    pub fn get_resource_mut<'a, T>(&self, world: &'a hecs::World) -> hecs::RefMut<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&mut T>(self.res_id)
            .expect("resource not found")
    }

    pub fn get_command_buffer<'a>(&'a self) -> RefMut<'a, hecs::CommandBuffer> {
        self.cmds.borrow_mut()
    }
}
