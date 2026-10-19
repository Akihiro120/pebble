use std::any::TypeId;

use crate::ecs::{
    condition::{ConditionSystem, IntoCondition},
    resources::Resources,
};

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

/// What a [`Schedule`](crate::ecs::schedule::Schedule) stores for one
/// registered system: its identity (used to resolve `.after`/`.before`
/// against), the boxed runnable, its tie-break priority, and any run
/// conditions gating it.
#[doc(hidden)]
pub struct ScheduledSystem {
    pub id: TypeId,
    pub system: Box<dyn System>,
    pub priority: i32,
    pub conditions: Vec<Box<dyn ConditionSystem>>,
}

/// A system bundled with `.after(...)`/`.before(...)` ordering constraints,
/// a `.priority(...)`, and any `.run_if(...)` conditions — produced by
/// [`IntoSystemConfig`] and consumed by
/// [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system).
pub struct SystemConfig<S, Params> {
    system: S,
    priority: i32,
    constraints: Vec<OrderConstraint>,
    conditions: Vec<Box<dyn ConditionSystem>>,
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

    /// Sets this system's priority, used to break ties between systems that
    /// have no `after`/`before` relationship to each other — higher runs
    /// first. Defaults to 0. An explicit `after`/`before` constraint always
    /// takes precedence over priority: priority only decides ordering
    /// between systems the schedule would otherwise be free to run in any
    /// order.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Gates this system on a predicate, re-evaluated every time the
    /// schedule runs — see [`condition`](crate::ecs::condition). Calling it
    /// more than once requires *all* the conditions to hold.
    ///
    /// A gated-out system still holds its place in the schedule, so another
    /// system's `.after(...)` on it is unaffected by whether it ran.
    pub fn run_if<C, Marker>(mut self, condition: C) -> Self
    where
        C: IntoCondition<Marker>,
    {
        self.conditions.push(Box::new(condition.into_condition()));
        self
    }

    /// Unpacks this config into what [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system)
    /// actually stores: the [`ScheduledSystem`], plus any ordering
    /// constraints to register against its identity.
    #[doc(hidden)]
    pub fn into_parts(self) -> (ScheduledSystem, Vec<(TypeId, TypeId)>) {
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
        (
            ScheduledSystem {
                id,
                system: Box::new(self.system.into_system()),
                priority: self.priority,
                conditions: self.conditions,
            },
            constraints,
        )
    }
}

/// A bare system is trivially "configured" with no ordering constraints and
/// priority 0 — this is what lets [`Schedule::add_system`](crate::ecs::schedule::Schedule::add_system)
/// accept either a plain system or one built via `.after(...)`/`.before(...)`/`.priority(...)`.
impl<S, Params> From<S> for SystemConfig<S, Params>
where
    S: IntoSystem<Params> + 'static,
{
    fn from(system: S) -> Self {
        SystemConfig {
            system,
            priority: 0,
            constraints: Vec::new(),
            conditions: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// Lets `.after(...)`/`.before(...)`/`.priority(...)` be called directly on
/// a system — a plain function, closure, or anything else [`IntoSystem`] is
/// implemented for — to declare where it must run relative to another
/// system in the same [`Schedule`](crate::ecs::schedule::Schedule), or how
/// it should be prioritized against unconstrained systems:
///
/// ```ignore
/// schedule
///     .add_system(spawn_enemies)
///     .add_system(move_enemies.after(spawn_enemies))
///     .add_system(render.after(move_enemies))
///     .add_system(hud.priority(10)); // runs before other unconstrained systems
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

    /// Wraps this system with a priority — see [`SystemConfig::priority`].
    fn priority(self, priority: i32) -> SystemConfig<Self, Params>;

    /// Wraps this system with a run condition — see
    /// [`SystemConfig::run_if`].
    fn run_if<C, Marker>(self, condition: C) -> SystemConfig<Self, Params>
    where
        C: IntoCondition<Marker>;
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

    fn priority(self, priority: i32) -> SystemConfig<Self, Params> {
        SystemConfig::from(self).priority(priority)
    }

    fn run_if<C, Marker>(self, condition: C) -> SystemConfig<Self, Params>
    where
        C: IntoCondition<Marker>,
    {
        SystemConfig::from(self).run_if(condition)
    }
}

/// A sequence of systems built with [`Chain::chain`] — e.g.
/// `(spawn_enemies, move_enemies, render).chain()` — that forces each
/// system to run strictly after the one before it in the tuple, on top of
/// whatever `.after(...)`/`.before(...)`/`.priority(...)` is applied to the
/// chain as a whole. Register it with
/// [`Schedule::add_systems`](crate::ecs::schedule::Schedule::add_systems)
/// (not `add_system` — a chain is more than one system).
///
/// `.after`/`.before` on a chain only need to constrain its first/last
/// system respectively — every other member already transitively depends on
/// that one through the chain's own internal ordering. `.priority` instead
/// applies to every member, since each one competes for its own slot in the
/// schedule as it individually becomes eligible to run, not just the first.
pub struct SystemChain {
    systems: Vec<ScheduledSystem>,
    constraints: Vec<(TypeId, TypeId)>,
}

impl SystemChain {
    fn first_id(&self) -> TypeId {
        self.systems[0].id
    }

    fn last_id(&self) -> TypeId {
        self.systems[self.systems.len() - 1].id
    }

    /// Constrains the whole chain to run after `other` — see
    /// [`SystemConfig::after`].
    pub fn after<S2, P2>(mut self, _other: S2) -> Self
    where
        S2: IntoSystem<P2> + 'static,
    {
        let first = self.first_id();
        self.constraints.push((first, TypeId::of::<S2>()));
        self
    }

    /// Constrains the whole chain to run before `other` — see
    /// [`SystemConfig::before`].
    pub fn before<S2, P2>(mut self, _other: S2) -> Self
    where
        S2: IntoSystem<P2> + 'static,
    {
        let last = self.last_id();
        self.constraints.push((TypeId::of::<S2>(), last));
        self
    }

    /// Sets every system in the chain to this priority — see
    /// [`SystemConfig::priority`].
    pub fn priority(mut self, priority: i32) -> Self {
        for system in &mut self.systems {
            system.priority = priority;
        }
        self
    }

    /// Gates every system in the chain on `condition` — see
    /// [`SystemConfig::run_if`].
    ///
    /// The condition is `Clone` because each member gets its own instance
    /// rather than sharing one: a chain member can be skipped by the
    /// schedule between two others, so there is no single point at which
    /// "the chain" is evaluated. For a pure predicate that is invisible.
    /// For a *stateful* one — a condition holding a
    /// [`Local`](crate::ecs::local::Local) — it means each member advances
    /// its own copy of that state, which is usually not what you want:
    /// gate the individual systems instead.
    pub fn run_if<C, Marker>(mut self, condition: C) -> Self
    where
        C: IntoCondition<Marker> + Clone,
    {
        for system in &mut self.systems {
            system.conditions.push(Box::new(condition.clone().into_condition()));
        }
        self
    }

    /// Unpacks this chain into what
    /// [`Schedule::add_systems`](crate::ecs::schedule::Schedule::add_systems)
    /// actually stores: each system's identity, boxed runnable, and
    /// priority, plus every ordering constraint (the chain's own internal
    /// links and any external `.after`/`.before`).
    #[doc(hidden)]
    pub fn into_parts(self) -> (Vec<ScheduledSystem>, Vec<(TypeId, TypeId)>) {
        (self.systems, self.constraints)
    }
}

/// Lets `.chain()` be called on a tuple of 2 or more systems to force them
/// to run in that exact relative order within a
/// [`Schedule`](crate::ecs::schedule::Schedule), regardless of the order
/// they (or other unrelated systems) are added in:
///
/// ```ignore
/// schedule.add_systems(
///     (spawn_enemies, move_enemies, render)
///         .chain()
///         .after(setup)
///         .priority(10),
/// );
/// ```
pub trait Chain<Marker> {
    fn chain(self) -> SystemChain;
}

macro_rules! impl_chain {
    ($($S:ident : $P:ident),+) => {
        impl<$($S, $P),+> Chain<($($P,)+)> for ($($S,)+)
        where
            $($S: IntoSystem<$P> + 'static,)+
        {
            #[allow(non_snake_case)]
            fn chain(self) -> SystemChain {
                let ($($S,)+) = self;
                let systems: Vec<ScheduledSystem> = vec![
                    $(ScheduledSystem {
                        id: TypeId::of::<$S>(),
                        system: Box::new($S.into_system()) as Box<dyn System>,
                        priority: 0,
                        conditions: Vec::new(),
                    },)+
                ];
                // consecutive pairs: the later system must run after the earlier one.
                let constraints = systems.windows(2).map(|w| (w[1].id, w[0].id)).collect();
                SystemChain { systems, constraints }
            }
        }
    };
}

impl_chain!(A: PA, B: PB);
impl_chain!(A: PA, B: PB, C: PC);
impl_chain!(A: PA, B: PB, C: PC, D: PD);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG, H: PH);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG, H: PH, I: PI);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG, H: PH, I: PI, J: PJ);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG, H: PH, I: PI, J: PJ, K: PK);
impl_chain!(A: PA, B: PB, C: PC, D: PD, E: PE, F: PF, G: PG, H: PH, I: PI, J: PJ, K: PK, L: PL);

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
