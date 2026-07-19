use std::cell::RefMut;
use std::ops::{Deref, DerefMut};

use crate::ecs::resources::Resources;

/// Immutable borrow of a singleton resource `T`.
///
/// Obtained as a system parameter; derefs to `T`.
pub struct Res<'a, T: hecs::Component> {
    pub(crate) data: hecs::Ref<'a, T>,
}

impl<'a, T: hecs::Component> Deref for Res<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// Mutable borrow of a singleton resource `T`.
///
/// Obtained as a system parameter; derefs to `T`.
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

/// Borrow of an ECS query result.
///
/// Obtained as a system parameter; derefs to [`hecs::QueryBorrow`].
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

/// Deferred world-mutation commands available as a system parameter.
///
/// Mutations are buffered and applied to the world after all systems in the
/// current stage have finished running.
pub struct Commands<'a> {
    buffer: RefMut<'a, hecs::CommandBuffer>,
    resource_entity: hecs::Entity,
}

impl<'a> Commands<'a> {
    /// Queue a resource insertion. Applied after the current stage finishes.
    pub fn insert_resource<T: hecs::Component>(&mut self, res: T) {
        self.buffer.insert_one(self.resource_entity, res);
    }

    /// Queue a resource removal. Applied after the current stage finishes.
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

/// Per-system persistent local state.
///
/// Unlike [`Res`]/[`ResMut`], a `Local<T>` is *not* shared through
/// [`Resources`] — each system gets its own private `T`, initialized with
/// [`Default::default`] the first time the system is registered, and
/// preserved across every subsequent run of that system.
///
/// Useful for counters, caches, or any state a single system needs to
/// remember without polluting the global resource set.
pub struct Local<'a, T: Default + Send + Sync + 'static> {
    data: &'a mut T,
}

impl<'a, T: Default + Send + Sync + 'static> Deref for Local<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Default + Send + Sync + 'static> DerefMut for Local<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

/// Trait implemented for each valid system parameter type.
///
/// The macro-generated [`impl_system!`] blanket implementations use this to
/// fetch each parameter from the world and resources before calling the system
/// function. `State` is per-system storage owned by the [`FunctionSystem`]
/// itself (as opposed to `Item`, which only lives for the duration of one
/// call) — this is what lets [`Local`] persist between runs.
pub trait SystemParam {
    type Item<'a>;
    type State: Default + 'static;
    fn fetch<'a>(
        state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a>;
}

impl<T> SystemParam for Res<'static, T>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Res<'a, T>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resource: &'a Resources,
    ) -> Self::Item<'a> {
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
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resource: &'a Resources,
    ) -> Self::Item<'a> {
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
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resource: &'a Resources,
    ) -> Self::Item<'a> {
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
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resource: &'a Resources,
    ) -> Self::Item<'a> {
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
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        _resources: &'a Resources,
    ) -> Self::Item<'a> {
        Query {
            borrow: world.query::<Q>(),
        }
    }
}

impl SystemParam for Commands<'static> {
    type Item<'a> = Commands<'a>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        _world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        Commands {
            buffer: resources.get_command_buffer(),
            resource_entity: resources.resource_entity,
        }
    }
}

impl SystemParam for &'static hecs::World {
    type Item<'a> = &'a hecs::World;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        _resources: &'a Resources,
    ) -> Self::Item<'a> {
        world
    }
}

impl SystemParam for &'static Resources {
    type Item<'a> = &'a Resources;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        _world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        resources
    }
}

impl<T> SystemParam for Local<'static, T>
where
    T: Default + Send + Sync + 'static,
{
    type Item<'a> = Local<'a, T>;
    type State = T;

    fn fetch<'a>(
        state: &'a mut Self::State,
        _world: &'a hecs::World,
        _resources: &'a Resources,
    ) -> Self::Item<'a> {
        Local { data: state }
    }
}

/// A type-erased, executable system.
pub trait System: 'static {
    fn run(&mut self, world: &hecs::World, resources: &Resources);
}

/// Type-erased wrapper around a system function, created by [`IntoSystem`].
///
/// Holds `State`, the tuple of each parameter's [`SystemParam::State`] — this
/// is where [`Local`] values actually live between calls to `run`.
pub struct FunctionSystem<F, Marker, State = ()> {
    pub func: F,
    state: State,
    _marker: std::marker::PhantomData<Marker>,
}

/// Converts a function (or closure) with valid system parameters into a
/// [`System`] that can be registered with [`App::add_system`](crate::app::App::add_system).
///
/// Implemented via the [`impl_system!`] macro for function arities 0–8.
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
            type System = FunctionSystem<T, ($($param,)*), ($($param::State,)*)>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    func: self,
                    state: Default::default(),
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<T, $($param),*> System for FunctionSystem<T, ($($param,)*), ($($param::State,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _world: &hecs::World, _resources: &Resources) {
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                (self.func)($($param::fetch($param, _world, _resources)),*);
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
