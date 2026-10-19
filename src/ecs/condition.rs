//! Run conditions — a predicate attached to a system, deciding each tick
//! whether it runs at all.
//!
//! A condition is an ordinary system-shaped function that returns `bool`:
//! it takes the same [`SystemParam`](crate::ecs::system_param::SystemParam)s
//! any system can take, including a [`Local`](crate::ecs::local::Local) for
//! state that persists between evaluations.
//!
//! ```ignore
//! use pebble::prelude::*;
//! use pebble::ecs::condition::{not, resource_exists};
//!
//! app.add_system(SystemStage::Update, draw_hud.run_if(resource_exists::<Hud>()))
//!    .add_system(SystemStage::Update, load_scene.run_if(not(resource_exists::<Scene>())))
//!    .add_system(SystemStage::Update, tick.run_if(|p: Option<Read<Paused>>| p.is_none()));
//! ```
//!
//! Conditions do not affect ordering: a skipped system still occupies its
//! place in the schedule, so `.after(...)` on another system holds whether
//! it actually ran or not.

use crate::ecs::{resources::Resources, system_param::SystemParam};

/// A runnable predicate — the type-erased form [`IntoCondition`] produces,
/// so conditions over different parameter lists can share one `Vec`.
pub trait ConditionSystem: 'static {
    fn eval(&mut self, world: &hecs::World, resources: &Resources) -> bool;
}

/// Wraps a predicate function into a [`ConditionSystem`], holding its
/// [`SystemParam::State`] between evaluations — so a condition can use
/// [`Local`](crate::ecs::local::Local) exactly as a system does.
pub struct FunctionCondition<F, Marker, State = ()> {
    func: F,
    state: State,
    _marker: std::marker::PhantomData<Marker>,
}

/// Implemented for any `bool`-returning function whose parameters are all
/// [`SystemParam`]s. There is no overlap with
/// [`IntoSystem`](crate::ecs::system_param::IntoSystem): `FnMut(A)` in that
/// trait's bounds means `FnMut(A) -> ()`, which a predicate is not.
pub trait IntoCondition<Marker> {
    type Condition: ConditionSystem;

    fn into_condition(self) -> Self::Condition;
}

/// Inverts a condition.
pub fn not<C, Marker>(condition: C) -> Not<C::Condition>
where
    C: IntoCondition<Marker>,
{
    Not(condition.into_condition())
}

/// True while `T` is present in [`Resources`] — the common case of guarding
/// a system on setup having happened, without an `Option<Read<T>>` and an
/// early return inside every such system.
pub fn resource_exists<T: 'static>() -> ResourceExists<T> {
    ResourceExists(std::marker::PhantomData)
}

/// The condition [`resource_exists`] builds.
///
/// A named type rather than a returned closure: the blanket
/// [`IntoCondition`] impl needs `for<'a> FnMut(P::Item<'a>) -> bool`, and an
/// `impl Trait` return type can only promise the one lifetime it names — so
/// a closure over `Option<Read<'a, T>>` is not usable as a condition however
/// it is written. Checking [`Resources`] directly is also simply less work
/// than borrowing the resource just to drop it again.
pub struct ResourceExists<T>(std::marker::PhantomData<fn() -> T>);

/// Marker separating the already-built conditions from the macro-generated
/// function impls, which are all keyed by a tuple of parameter types. Two
/// distinct markers are what keeps the blanket impl below from overlapping
/// them: the compiler cannot otherwise rule out a condition type that also
/// happens to implement `FnMut() -> bool`.
pub struct BuiltinCondition;

/// Anything that is already a [`ConditionSystem`] converts to itself. This
/// is what lets [`not`]/[`ConditionExt::and`]/[`ConditionExt::or`] — whose
/// outputs are conditions, not functions — be passed straight back to
/// `.run_if(...)` and nested into each other.
impl<C: ConditionSystem> IntoCondition<BuiltinCondition> for C {
    type Condition = C;

    fn into_condition(self) -> Self::Condition {
        self
    }
}

impl<T> Clone for ResourceExists<T> {
    fn clone(&self) -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: 'static> ConditionSystem for ResourceExists<T> {
    fn eval(&mut self, _world: &hecs::World, resources: &Resources) -> bool {
        resources.contains::<T>()
    }
}

/// The inverse of a condition — see [`not`].
pub struct Not<C>(C);

impl<C: ConditionSystem> ConditionSystem for Not<C> {
    fn eval(&mut self, world: &hecs::World, resources: &Resources) -> bool {
        !self.0.eval(world, resources)
    }
}

/// Both conditions hold. Short-circuits: if the first is false the second is
/// not evaluated, so a stateful second condition will not tick that run.
pub struct And<A, B>(A, B);

impl<A: ConditionSystem, B: ConditionSystem> ConditionSystem for And<A, B> {
    fn eval(&mut self, world: &hecs::World, resources: &Resources) -> bool {
        self.0.eval(world, resources) && self.1.eval(world, resources)
    }
}

/// Either condition holds. Short-circuits, as [`And`] does.
pub struct Or<A, B>(A, B);

impl<A: ConditionSystem, B: ConditionSystem> ConditionSystem for Or<A, B> {
    fn eval(&mut self, world: &hecs::World, resources: &Resources) -> bool {
        self.0.eval(world, resources) || self.1.eval(world, resources)
    }
}

/// Lets `.and(...)`/`.or(...)` be called directly on a condition to combine
/// it with another, rather than nesting calls.
pub trait ConditionExt<Marker>: IntoCondition<Marker> + Sized {
    fn and<C2, M2>(self, other: C2) -> And<Self::Condition, C2::Condition>
    where
        C2: IntoCondition<M2>,
    {
        And(self.into_condition(), other.into_condition())
    }

    fn or<C2, M2>(self, other: C2) -> Or<Self::Condition, C2::Condition>
    where
        C2: IntoCondition<M2>,
    {
        Or(self.into_condition(), other.into_condition())
    }
}

impl<T, Marker> ConditionExt<Marker> for T where T: IntoCondition<Marker> {}

/// An already-erased condition is itself a [`ConditionSystem`], so a boxed
/// one can still be combined with [`And`]/[`Or`]/[`Not`].
impl ConditionSystem for Box<dyn ConditionSystem> {
    fn eval(&mut self, world: &hecs::World, resources: &Resources) -> bool {
        (**self).eval(world, resources)
    }
}

macro_rules! impl_condition {
    ($($param:ident),*) => {
        impl<T, $($param),*> IntoCondition<($($param,)*)> for T
        where
            T: FnMut($($param),*) -> bool
                + for<'a> FnMut($($param::Item<'a>),*) -> bool
                + 'static,
            $($param: SystemParam + 'static),*
        {
            type Condition = FunctionCondition<T, ($($param,)*), ($($param::State,)*)>;

            fn into_condition(self) -> Self::Condition {
                FunctionCondition {
                    func: self,
                    state: Default::default(),
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<T, $($param),*> ConditionSystem
            for FunctionCondition<T, ($($param,)*), ($($param::State,)*)>
        where
            T: FnMut($($param),*) -> bool
                + for<'a> FnMut($($param::Item<'a>),*) -> bool
                + 'static,
            $($param: SystemParam + 'static),*
        {
            fn eval(&mut self, _world: &hecs::World, _resources: &Resources) -> bool {
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                (self.func)($($param::fetch(_world, _resources, $param)),*)
            }
        }
    };
}

impl_condition!();
impl_condition!(A);
impl_condition!(A, B);
impl_condition!(A, B, C);
impl_condition!(A, B, C, D);
impl_condition!(A, B, C, D, E);
impl_condition!(A, B, C, D, E, F);
impl_condition!(A, B, C, D, E, F, G);
impl_condition!(A, B, C, D, E, F, G, H);
