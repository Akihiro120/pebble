use std::cell::RefMut;
use std::ops::{Deref, DerefMut};

use crate::resources::Resources;

pub struct Res<'a, T: hecs::Component> {
    pub(crate) data: hecs::Ref<'a, T>,
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

pub struct Query<'a, Q: hecs::Query> {
    borrow: hecs::QueryBorrow<'a, Q>,
}

impl<'a, Q: hecs::Query> Deref for Query<'a, Q> {
    type Target = hecs::QueryBorrow<'a, Q>;
    fn deref(&self) -> &Self::Target {
        &self.borrow
    }
}

impl<'a, Q: hecs::Query> DerefMut for Query<'a, Q> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.borrow
    }
}

pub struct Commands<'a> {
    buffer: RefMut<'a, hecs::CommandBuffer>,
    resource_entity: hecs::Entity,
}

impl<'a> Commands<'a> {
    pub fn insert_resource<T: hecs::Component>(&mut self, res: T) {
        self.buffer.insert_one(self.resource_entity, res);
    }

    pub fn remove_resource<T: hecs::Component>(&mut self) {
        self.buffer.remove_one::<T>(self.resource_entity);
    }
}

impl<'a> Deref for Commands<'a> {
    type Target = hecs::CommandBuffer;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<'a> DerefMut for Commands<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
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

impl<T> SystemParam for Option<Res<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<Res<'a, T>>;

    fn fetch<'a>(world: &'a hecs::World, resource: &'a Resources) -> Self::Item<'a> {
        if resource.has_resource::<T>(world) {
            return Some(Res {
                data: resource.get_resource(world),
            });
        }

        None
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

impl<T> SystemParam for Option<ResMut<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<ResMut<'a, T>>;

    fn fetch<'a>(world: &'a hecs::World, resource: &'a Resources) -> Self::Item<'a> {
        if resource.has_resource::<T>(world) {
            return Some(ResMut {
                data: resource.get_resource_mut(world),
            });
        }

        None
    }
}

impl<Q> SystemParam for Query<'static, Q>
where
    Q: hecs::Query + 'static,
{
    type Item<'a> = Query<'a, Q>;

    fn fetch<'a>(world: &'a hecs::World, _resources: &'a Resources) -> Self::Item<'a> {
        Query {
            borrow: world.query::<Q>(),
        }
    }
}

impl SystemParam for Commands<'static> {
    type Item<'a> = Commands<'a>;

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources) -> Self::Item<'a> {
        Commands {
            buffer: resources.get_command_buffer(),
            resource_entity: resources.resource_entity,
        }
    }
}

impl SystemParam for &'static hecs::World {
    type Item<'a> = &'a hecs::World;

    fn fetch<'a>(world: &'a hecs::World, _resources: &'a Resources) -> Self::Item<'a> {
        world
    }
}

impl SystemParam for &'static Resources {
    type Item<'a> = &'a Resources;

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources) -> Self::Item<'a> {
        resources
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
        impl<T, $($param),*> IntoSystem<($($param,)*)> for T
        where
            T: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            for<'a> &'a mut T: FnMut($($param),*),
            $($param: SystemParam + 'static),*
        {
            type System = FunctionSystem<T, ($($param,)*)>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    func: self,
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<T, $($param),*> System for FunctionSystem<T, ($($param,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _world: &hecs::World, _resources: &Resources) {
                (self.func)($($param::fetch(_world, _resources)),*);
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
impl_system!(A, B, C, D, E, F);
impl_system!(A, B, C, D, E, F, G);
impl_system!(A, B, C, D, E, F, G, H);
