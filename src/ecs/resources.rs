use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

use crate::ecs::system_param::SystemParam;

#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, RefCell<Box<dyn Any>>>,
}

impl Resources {
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.map
            .insert(TypeId::of::<T>(), RefCell::new(Box::new(value)));
    }

    pub fn get<T: 'static>(&self) -> Ref<'_, T> {
        let cell = self
            .map
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Resource not found: {}", std::any::type_name::<T>()));

        Ref::map(cell.borrow(), |b| b.downcast_ref::<T>().unwrap())
    }

    pub fn get_mut<T: 'static>(&self) -> RefMut<'_, T> {
        let cell = self
            .map
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Resource not found: {}", std::any::type_name::<T>()));

        RefMut::map(cell.borrow_mut(), |b| b.downcast_mut::<T>().unwrap())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .map(|cell| *cell.into_inner().downcast::<T>().unwrap())
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

pub struct Read<'a, T: 'static> {
    inner: Ref<'a, T>,
}

impl<'a, T> std::ops::Deref for Read<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct Write<'a, T: 'static> {
    inner: RefMut<'a, T>,
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
        _world: &'a hecs::World,
        resource: &'a Resources,
        _state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        if resource.contains::<T>() {
            return Some(Read {
                inner: resource.get::<T>(),
            });
        }

        None
    }
}

impl<T> SystemParam for Option<Write<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<Write<'a, T>>;
    type State = ();

    fn fetch<'a>(
        _world: &'a hecs::World,
        resource: &'a Resources,
        _state: &'a mut Self::State,
    ) -> Self::Item<'a> {
        if resource.contains::<T>() {
            return Some(Write {
                inner: resource.get_mut(),
            });
        }

        None
    }
}
