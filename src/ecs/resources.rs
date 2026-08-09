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
}

pub struct Res<'a, T: 'static> {
    inner: Ref<'a, T>,
}

impl<'a, T> std::ops::Deref for Res<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ResMut<'a, T: 'static> {
    inner: RefMut<'a, T>,
}

impl<'a, T> std::ops::Deref for ResMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> std::ops::DerefMut for ResMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: 'static> SystemParam for Res<'_, T> {
    type Item<'a> = Res<'a, T>;

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources) -> Self::Item<'a> {
        Res {
            inner: resources.get::<T>(),
        }
    }
}

impl<T: 'static> SystemParam for ResMut<'_, T> {
    type Item<'a> = ResMut<'a, T>;

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources) -> Self::Item<'a> {
        ResMut {
            inner: resources.get_mut::<T>(),
        }
    }
}
