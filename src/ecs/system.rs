use std::cell::RefMut;
use std::ops::{Deref, DerefMut};

use crate::ecs::resources::Resources;

/// A resource requirement declared by a [`SystemParam`]/[`System`], carrying
/// both a human-readable name and a way to check presence dynamically
/// (needed because [`System::requires`] is type-erased — the concrete `T`
/// is only known where the check is constructed, inside each `SystemParam`
/// impl).
#[derive(Clone, Copy)]
pub struct RequiredResource {
    pub name: &'static str,
    pub present: fn(&hecs::World, &Resources) -> bool,
}

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
///
/// # Iterating
///
/// `Query` implements `IntoIterator` for `&mut Query`, so you can iterate it
/// directly without going through `Deref`:
///
/// ```ignore
/// fn move_system(mut q: Query<(&mut Position, &Velocity)>) {
///     for (entity, (pos, vel)) in &mut q {
///         pos.x += vel.x;
///         pos.y += vel.y;
///     }
/// }
/// ```
///
/// # Single-entity lookups
///
/// Use [`Query::get`] to fetch components for one known `Entity` without
/// scanning the whole result set, and [`Query::single`] /
/// [`Query::get_single`] when you expect exactly one match (e.g. "the
/// player", "the active camera").
pub struct Query<'a, Q: hecs::Query> {
    world: &'a hecs::World,
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

impl<'q, Q: hecs::Query> IntoIterator for &'q mut Query<'_, Q> {
    type Item = Q::Item<'q>;
    type IntoIter = hecs::QueryIter<'q, Q>;

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.borrow).into_iter()
    }
}

impl<'a, Q: hecs::Query> Query<'a, Q> {
    /// Look up a single entity's components for this query. Returns
    /// `None` if the entity doesn't exist or doesn't match `Q`.
    pub fn get(&self, entity: hecs::Entity) -> hecs::QueryOne<'_, Q> {
        self.world.query_one::<Q>(entity)
    }

    /// Filter this query to only entities that ALSO have component `R`,
    /// without `R` itself being part of the yielded items. Consumes
    /// `self` — matches hecs's own `QueryBorrow::with` signature.
    pub fn with<R: hecs::Query>(self) -> hecs::QueryBorrow<'a, hecs::With<Q, R>> {
        self.borrow.with::<R>()
    }

    /// Filter this query to only entities that do NOT have component `R`.
    /// Consumes `self`, same reasoning as `with`.
    pub fn without<R: hecs::Query>(self) -> hecs::QueryBorrow<'a, hecs::Without<Q, R>> {
        self.borrow.without::<R>()
    }

    /// Return the single entity's components for this query.
    ///
    /// Panics if there isn't exactly one match. Intended for singleton-style
    /// queries (the player, the active camera, ...) where zero or multiple
    /// matches indicate a bug. See [`Query::get_single`] for a
    /// non-panicking version. Include `hecs::Entity` in `Q` if you need the
    /// id alongside the components.
    pub fn single(&mut self) -> Q::Item<'_> {
        self.get_single()
            .expect("Query::single: expected exactly one matching entity")
    }

    /// Like [`Query::single`], but returns `None` instead of panicking when
    /// there isn't exactly one match.
    pub fn get_single(&mut self) -> Option<Q::Item<'_>> {
        let mut iter = self.borrow.iter();
        let first = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(first)
    }
}

/// Deferred world-mutation commands available as a system parameter.
///
/// Mutations are buffered and applied to the world after all systems in the
/// current stage have finished running.
///
/// Resource insertions immediately bump the [`Resources`] generation counter so
/// that the convergence loop in [`App`](crate::app::App) can detect them without
/// needing to inspect the world after every flush.
pub struct Commands<'a> {
    buffer: RefMut<'a, hecs::CommandBuffer>,
    resource_entity: hecs::Entity,
    /// Held so `insert_resource` can bump the generation counter at queue time.
    resources: &'a Resources,
}

impl<'a> Commands<'a> {
    /// Queue a resource insertion. Applied after the current stage finishes.
    ///
    /// Bumps the [`Resources`] generation counter immediately so the
    /// convergence loop knows another pass is needed even before the command
    /// buffer is flushed.
    pub fn insert_resource<T: hecs::Component>(&mut self, res: T) {
        self.buffer.insert_one(self.resource_entity, res);
        self.resources.bump_generation();
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

    /// Resource types this parameter unconditionally needs present to avoid
    /// panicking. Used by [`App`](crate::app::App) to validate — before
    /// running a non-convergent stage's systems — that every hard
    /// requirement is already satisfied, failing fast with a clear message
    /// instead of panicking deep inside whichever system happens to run
    /// first.
    ///
    /// Empty by default; only hard requirements (bare [`Res`]/[`ResMut`])
    /// contribute an entry. `Option<Res<T>>`/`Option<ResMut<T>>` tolerate
    /// absence and deliberately opt out of this check.
    fn requires() -> Vec<RequiredResource> {
        Vec::new()
    }
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

    fn requires() -> Vec<RequiredResource> {
        vec![RequiredResource {
            name: std::any::type_name::<T>(),
            present: |world, resources| resources.has_resource::<T>(world),
        }]
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

    fn requires() -> Vec<RequiredResource> {
        vec![RequiredResource {
            name: std::any::type_name::<T>(),
            present: |world, resources| resources.has_resource::<T>(world),
        }]
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
            world: world,
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
            resources,
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

    /// Resource types this system needs present, derived automatically from
    /// its bare [`Res`]/[`ResMut`] parameters. [`App`](crate::app::App)
    /// checks these before running a non-convergent stage's systems and
    /// panics with a clear message naming the missing resource(s) rather
    /// than letting a param fetch panic deep inside whichever system happens
    /// to run first.
    fn requires(&self) -> Vec<RequiredResource> {
        Vec::new()
    }

    /// Human-readable identifier for this system, used in error/trace output
    /// so a missing-resource failure can be pinned to the system that needs
    /// it instead of just the resource name. Defaults to the type name of
    /// the [`System`] impl; [`FunctionSystem`] overrides this with the name
    /// of the wrapped function/closure, which is far more legible.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
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

            fn requires(&self) -> Vec<RequiredResource> {
                let mut _v = Vec::new();
                $(_v.extend($param::requires());)*
                _v
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<T>()
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
impl_system!(A, B, C, D, E, F, G, H, I);
impl_system!(A, B, C, D, E, F, G, H, I, J);
impl_system!(A, B, C, D, E, F, G, H, I, J, K);
impl_system!(A, B, C, D, E, F, G, H, I, J, K, L);
