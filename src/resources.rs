pub struct Resources {
    res_id: hecs::Entity,
}

impl Resources {
    pub fn new(world: &mut hecs::World) -> Self {
        Self {
            res_id: world.spawn(()),
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

    pub fn get_resource_mut<'a, T>(&self, world: &'a hecs::World) -> hecs::RefMut<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&mut T>(self.res_id)
            .expect("resource not found")
    }
}
