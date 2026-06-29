use std::cell::RefMut;
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

pub struct Query<'a, Q: hecs::Query> {
    borrow: hecs::QueryBorrow<'a, Q>,
}

// impl<'a, Q: hecs::Query> Query<'a, Q> {
//     pub fn iter(&mut self) -> hecs::QueryIter<'_, Q> {
//         self.borrow.iter()
//     }
// }

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
        }
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
