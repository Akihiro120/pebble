use std::ops::{Deref, DerefMut};

use crate::resources::Resources;

pub struct Res<'a, T: hecs::Component> {
    data: hecs::Ref<'a, T>,
}

impl<'a, T: hecs::Component> Deref for Res<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub struct ResMut<'a, T: hecs::Component> {
    data: hecs::RefMut<'a, T>,
}

impl<'a, T: hecs::Component> Deref for ResMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T: hecs::Component> DerefMut for ResMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct Query<Q: hecs::Query> {
    _marker: std::marker::PhantomData<Q>,
}

trait SystemParam {
    type Item<'a>;
    fn fetch<'a>(world: &'a hecs::World, resources: &'a Resources) -> Self::Item<'a>;
}

impl<T> SystemParam for Res<'static, T>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Res<'a, T>;

    fn fetch<'a>(world: &'a hecs::World, resource: &'a Resources) -> Self::Item<'a> {
        Res {
            data: resource.get_resource(world),
        }
    }
}

impl<T> SystemParam for ResMut<'static, T>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = ResMut<'a, T>;

    fn fetch<'a>(world: &'a hecs::World, resource: &'a Resources) -> Self::Item<'a> {
        ResMut {
            data: resource.get_resource_mut(world),
        }
    }
}

impl<Q> SystemParam for Query<Q>
where
    Q: hecs::Query,
{
    type Item<'a> = hecs::QueryBorrow<'a, Q>;

    fn fetch<'a>(world: &'a hecs::World, _resource: &'a Resources) -> Self::Item<'a> {
        world.query::<Q>()
    }
}

pub trait System: 'static {
    fn run(&mut self, world: &hecs::World, resources: &Resources);
}

pub struct FunctionSystem<F, Marker> {
    pub func: F,
    _marker: std::marker::PhantomData<Marker>,
}

pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
}

macro_rules! impl_system {
    ($($param:ident),*) => {
        impl<F, $($param),*> IntoSystem<($($param,)*)> for F
        where
            F: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            for<'a> &'a mut F: FnMut($($param),*),
            $($param: SystemParam + 'static),*
        {
            type System = FunctionSystem<F, ($($param,)*)>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    func: self,
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<F, $($param),*> System for FunctionSystem<F, ($($param,)*)>
        where
            F: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, world: &hecs::World, resources: &Resources) {
                (self.func)($($param::fetch(world, resources)),*);
            }
        }
    };
}

impl_system!();
impl_system!(A);
impl_system!(A, B);
impl_system!(A, B, C);
impl_system!(A, B, C, D);
impl_system!(A, B, C, D, E);
impl_system!(A, B, C, D, E, G);
impl_system!(A, B, C, D, E, G, H);
