use std::cell::{Cell, RefCell, RefMut};

/// Container for singleton resources stored inside the ECS world.
///
/// All resources live on a single hidden entity so they participate in the
/// same borrow-checking rules as regular components. [`Resources`] is passed
/// to every system alongside the [`hecs::World`].
pub struct Resources {
    pub(crate) resource_entity: hecs::Entity,
    cmds: RefCell<hecs::CommandBuffer>,
    generation: Cell<u64>,
}

impl Resources {
    /// Create a new `Resources` container, spawning the internal resource entity.
    pub fn new(world: &mut hecs::World) -> Self {
        Self {
            resource_entity: world.spawn(()),
            cmds: RefCell::new(hecs::CommandBuffer::default()),
            generation: Cell::new(0),
        }
    }

    /// Insert or replace a resource of type `T`.
    pub fn insert_resource<T>(&mut self, world: &mut hecs::World, res: T)
    where
        T: hecs::Component,
    {
        world.insert_one(self.resource_entity, res).ok();
        self.generation.set(self.generation.get() + 1);
    }

    /// Borrow resource `T`, panicking if it is not present.
    pub fn get_resource<'a, T>(&self, world: &'a hecs::World) -> hecs::Ref<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&T>(self.resource_entity)
            .unwrap_or_else(|_| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    /// Mutably borrow resource `T`, panicking if it is not present.
    pub fn get_resource_mut<'a, T>(&self, world: &'a hecs::World) -> hecs::RefMut<'a, T>
    where
        T: hecs::Component,
    {
        world
            .get::<&mut T>(self.resource_entity)
            .unwrap_or_else(|_| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    /// Returns `true` if resource `T` is currently present.
    pub fn has_resource<T>(&self, world: &hecs::World) -> bool
    where
        T: hecs::Component,
    {
        if let Ok(_) = world.get::<&T>(self.resource_entity) {
            return true;
        }

        false
    }

    /// Borrow the shared command buffer used to defer world mutations.
    pub fn get_command_buffer<'a>(&'a self) -> RefMut<'a, hecs::CommandBuffer> {
        self.cmds.borrow_mut()
    }

    /// Insert resource `T` only if it is not already present.
    ///
    /// Returns `true` if the resource was inserted, `false` if it already existed.
    pub fn try_insert<T>(&mut self, world: &mut hecs::World, res: T) -> bool
    where
        T: hecs::Component,
    {
        if self.has_resource::<T>(world) {
            return false;
        }
        world.insert_one(self.resource_entity, res).ok();
        self.generation.set(self.generation.get() + 1);
        true
    }

    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// Manually bump the generation counter.
    ///
    /// Called by [`App`](crate::app::App) after flushing the command buffer when
    /// it detects that new resources were inserted via [`Commands`](crate::ecs::system::Commands)
    /// (which bypasses the normal [`insert_resource`](Self::insert_resource) path).
    pub(crate) fn bump_generation(&self) {
        self.generation.set(self.generation.get() + 1);
    }
}
