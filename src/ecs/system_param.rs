use std::any::TypeId;

use crate::ecs::resources::Resources;

/// Anything that can be fetched as a system function parameter —
/// implemented for [`Read`](crate::ecs::resources::Read)/[`Write`](crate::ecs::resources::Write),
/// [`Query`](crate::ecs::query::Query), [`Local`](crate::ecs::local::Local),
/// [`Commands`](crate::ecs::commands::Commands), tuples of `SystemParam`s
/// (so a function can take several), and a few others. You generally don't
/// implement this yourself unless you're adding a new kind of parameter.
pub trait SystemParam {
    /// The value actually handed to the system function.
    type Item<'a>;
    /// Per-system persistent state (e.g. [`Local`](crate::ecs::local::Local)'s
    /// stored value) — `()` for anything stateless.
    type State: Default + 'static;

    fn fetch<'a>(
        world: &'a hecs::World,
        resources: &'a Resources,
        state: &'a mut Self::State,
    ) -> Self::Item<'a>;
}

impl<'a> SystemParam for &'a hecs::World {
    type Item<'w> = &'w hecs::World;
    type State = ();

    fn fetch<'w>(
        world: &'w hecs::World,
        _resources: &'w Resources,
        _state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        world
    }
}

impl<'a> SystemParam for &'a Resources {
    type Item<'w> = &'w Resources;
    type State = ();

    fn fetch<'w>(
        _world: &'w hecs::World,
        resources: &'w Resources,
        _state: &'w mut Self::State,
    ) -> Self::Item<'w> {
        resources
    }
}

/// A runnable system — the type-erased form `IntoSystem` produces, so
/// different systems (different parameter lists) can live in the same
/// `Vec<Box<dyn System>>`.
pub trait System: 'static {
    fn run(&mut self, world: &hecs::World, resources: &Resources);
}

/// Wraps a plain function into a [`System`], holding its per-call
/// [`SystemParam::State`] between runs.
pub struct FunctionSystem<F, Marker, State = ()> {
    pub func: F,
    state: State,
    _marker: std::marker::PhantomData<Marker>,
}

/// Implemented for any function whose parameters are all [`SystemParam`]s —
/// this is what lets a plain `fn my_system(time: Read<Time>)` be passed
/// directly to `add_system`.
pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
}

/// A relative-ordering rule attached to a system via
/// [`IntoSystemConfig::after`]/[`IntoSystemConfig::before`], keyed by the
/// other system's own type — no runtime label needed, since a distinct
/// function or closure is already a distinct type.
enum OrderConstraint {
    After(TypeId),
    Before(TypeId),
}

/// A system bundled with `.after(...)`/`.before(...)` ordering constraints,
/// produced by [`IntoSystemConfig::after`]/[`IntoSystemConfig::before`] and
/// consumed by [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system).
pub struct SystemConfig<S, Params> {
    system: S,
    constraints: Vec<OrderConstraint>,
    _marker: std::marker::PhantomData<fn() -> Params>,
}

impl<S, Params> SystemConfig<S, Params>
where
    S: IntoSystem<Params> + 'static,
{
    /// Adds a constraint that this system must run after `other`. `other`
    /// need not be added to the schedule yet — only its type is used, to
    /// look it up when the schedule's order is next computed.
    pub fn after<S2, P2>(mut self, _other: S2) -> Self
    where
        S2: IntoSystem<P2> + 'static,
    {
        self.constraints.push(OrderConstraint::After(TypeId::of::<S2>()));
        self
    }

    /// Adds a constraint that this system must run before `other`. `other`
    /// need not be added to the schedule yet — only its type is used, to
    /// look it up when the schedule's order is next computed.
    pub fn before<S2, P2>(mut self, _other: S2) -> Self
    where
        S2: IntoSystem<P2> + 'static,
    {
        self.constraints.push(OrderConstraint::Before(TypeId::of::<S2>()));
        self
    }

    /// Unpacks this config into what [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system)
    /// actually stores: the system's identity, its boxed runnable, and any
    /// ordering constraints to register against that identity.
    #[doc(hidden)]
    pub fn into_parts(self) -> (TypeId, Box<dyn System>, Vec<(TypeId, TypeId)>) {
        let id = TypeId::of::<S>();
        // `(dependent, dependency)` — dependent must run after dependency.
        let constraints = self
            .constraints
            .into_iter()
            .map(|constraint| match constraint {
                OrderConstraint::After(dependency) => (id, dependency),
                OrderConstraint::Before(dependent) => (dependent, id),
            })
            .collect();
        (id, Box::new(self.system.into_system()), constraints)
    }
}

/// A bare system is trivially "configured" with no ordering constraints —
/// this is what lets [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system)
/// accept either a plain system or one built via `.after(...)`/`.before(...)`.
impl<S, Params> From<S> for SystemConfig<S, Params>
where
    S: IntoSystem<Params> + 'static,
{
    fn from(system: S) -> Self {
        SystemConfig {
            system,
            constraints: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// Lets `.after(...)`/`.before(...)` be called directly on a system —
/// a plain function, closure, or anything else [`IntoSystem`] is
/// implemented for — to declare where it must run relative to another
/// system in the same [`Schedule`](crate::ecs::schedule::Schedule):
///
/// ```ignore
/// schedule
///     .add_system(spawn_enemies)
///     .add_system(move_enemies.after(spawn_enemies))
///     .add_system(render.after(move_enemies));
/// ```
pub trait IntoSystemConfig<Params>: IntoSystem<Params> + Sized {
    /// Wraps this system with a constraint that it must run after `other`.
    fn after<S2, P2>(self, other: S2) -> SystemConfig<Self, Params>
    where
        S2: IntoSystem<P2> + 'static;

    /// Wraps this system with a constraint that it must run before `other`.
    fn before<S2, P2>(self, other: S2) -> SystemConfig<Self, Params>
    where
        S2: IntoSystem<P2> + 'static;
}

impl<T, Params> IntoSystemConfig<Params> for T
where
    T: IntoSystem<Params> + 'static,
{
    fn after<S2, P2>(self, other: S2) -> SystemConfig<Self, Params>
    where
        S2: IntoSystem<P2> + 'static,
    {
        SystemConfig::from(self).after(other)
    }

    fn before<S2, P2>(self, other: S2) -> SystemConfig<Self, Params>
    where
        S2: IntoSystem<P2> + 'static,
    {
        SystemConfig::from(self).before(other)
    }
}

macro_rules! impl_system {
    ($($param:ident),*) => {
        impl<T, $($param),*> IntoSystem<($($param,)*)> for T
        where
            T: FnMut($($param),*) + for<'a> FnMut($($param::Item<'a>),*) + 'static,
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
            T: FnMut($($param),*) + for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _world: &hecs::World, _resources: &Resources) {
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                (self.func)($($param::fetch(_world, _resources, $param)),*);
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
