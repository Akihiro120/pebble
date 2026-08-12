use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

use crate::ecs::system_param::SystemParam;

/// Singleton storage for app-wide resources, keyed by type. One value per
/// type, borrow-checked at runtime via `RefCell` rather than the compiler —
/// [`Read`]/[`Write`] are the system-facing way to borrow from this.
#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, RefCell<Box<dyn Any>>>,
}

impl Resources {
    fn cell<T: 'static>(&self) -> &RefCell<Box<dyn Any>> {
        self.map
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Resource not found: {}", std::any::type_name::<T>()))
    }

    /// Inserts a value, replacing any existing one of the same type.
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.map
            .insert(TypeId::of::<T>(), RefCell::new(Box::new(value)));
    }

    /// Borrows `T` immutably. Panics if it isn't present.
    pub fn get<T: 'static>(&self) -> Ref<'_, T> {
        Ref::map(self.cell::<T>().borrow(), |b| b.downcast_ref::<T>().unwrap())
    }

    /// Borrows `T` mutably. Panics if it isn't present.
    pub fn get_mut<T: 'static>(&self) -> RefMut<'_, T> {
        RefMut::map(self.cell::<T>().borrow_mut(), |b| b.downcast_mut::<T>().unwrap())
    }

    /// Removes and returns `T`, if present.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .map(|cell| *cell.into_inner().downcast::<T>().unwrap())
    }

    /// Whether a value of type `T` is currently stored.
    pub fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

/// An immutable resource borrow, fetched automatically as a system
/// parameter. Panics at fetch time if `T` isn't in [`Resources`] — use
/// `Option<Read<T>>` for a resource that might not exist.
pub struct Read<'a, T: 'static> {
    pub(crate) inner: Ref<'a, T>,
}

impl<'a, T> std::ops::Deref for Read<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A mutable resource borrow, fetched automatically as a system parameter.
/// Panics at fetch time if `T` isn't in [`Resources`] — use
/// `Option<Write<T>>` for a resource that might not exist.
pub struct Write<'a, T: 'static> {
    pub(crate) inner: RefMut<'a, T>,
}

impl<'a, T> std::ops::Deref for Write<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> std::ops::DerefMut for Write<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: 'static> SystemParam for Read<'_, T> {
    type Item<'a> = Read<'a, T>;
    type State = ();

    fn fetch<'a>(
        _world: &'a hecs::World,
        resources: &'a Resources,
        _state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        Read {
            inner: resources.get::<T>(),
        }
    }
}

impl<T: 'static> SystemParam for Write<'_, T> {
    type Item<'a> = Write<'a, T>;
    type State = ();

    fn fetch<'a>(
        _world: &'a hecs::World,
        resources: &'a Resources,
        _state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        Write {
            inner: resources.get_mut::<T>(),
        }
    }
}

impl<T> SystemParam for Option<Read<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<Read<'a, T>>;
    type State = ();

    fn fetch<'a>(
        world: &'a hecs::World,
        resources: &'a Resources,
        state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        resources.contains::<T>().then(|| Read::fetch(world, resources, state))
    }
}

impl<T> SystemParam for Option<Write<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<Write<'a, T>>;
    type State = ();

    fn fetch<'a>(
        world: &'a hecs::World,
        resources: &'a Resources,
        state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        resources.contains::<T>().then(|| Write::fetch(world, resources, state))
    }
}
