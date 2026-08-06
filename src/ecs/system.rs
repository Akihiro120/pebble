use std::cell::RefMut;
use std::ops::{Deref, DerefMut};

use crate::ecs::resources::Resources;
// A `.detach()`-ed system's returned future must satisfy this bound —
// `BackgroundTasks::spawn_async`'s own bound, since that's what ends up
// driving it.
use crate::threading::SpawnableFuture;

/// A resource requirement declared by a [`SystemParam`]/[`System`], carrying
/// a human-readable name, the resource's [`TypeId`](std::any::TypeId) (so
/// [`App`](crate::app::App) can check it against
/// [`RequiredResources`](crate::assets::required::RequiredResources) —
/// resources some plugin has declared it eventually provides, e.g. an async
/// GPU backend or a [`LazyResource`](crate::assets::singleton_asset::LazyResource) —
/// and a way to check presence dynamically (needed because
/// [`System::requires`] is type-erased — the concrete `T` is only known
/// where the check is constructed, inside each `SystemParam` impl).
#[derive(Clone, Copy)]
pub struct RequiredResource {
    pub name: &'static str,
    pub type_id: std::any::TypeId,
    pub present: fn(&hecs::World, &Resources) -> bool,
    /// Overrides `App`'s generic "call `app.provides::<T>()` or
    /// `App::add_resource`" advice when this resource has its own, more
    /// specific registration path (e.g. `Events<T>` — the actual fix is
    /// `app.add_event::<T>()`, not a manual `provides` call). `None` falls
    /// back to the generic advice, appropriate for a plain `Res<T>`/`ResMut<T>`
    /// on an arbitrary user resource type.
    pub hint: Option<&'static str>,
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

/// Borrow of an ECS query result — a curated wrapper around `hecs`'s query types, not a
/// transparent passthrough to them: no `hecs::*` type appears in any method here, and nothing
/// leaks through beyond what's listed below.
///
/// # Iterating
///
/// [`iter`](Self::iter) returns a plain `Iterator`, so the usual adapters (`.filter(...)`,
/// `.map(...)`, `.take(...)`, `.collect()`, ...) all compose directly — reach for `.filter(...)`
/// for a predicate over the yielded *values* (health below a threshold, say); [`with`](Self::with)/
/// [`without`](Self::without) below are for filtering by which *components* an entity has, not
/// their values.
///
/// ```ignore
/// fn move_system(mut q: Query<(&mut Position, &Velocity)>) {
///     for (pos, vel) in q.iter() {
///         pos.x += vel.x;
///         pos.y += vel.y;
///     }
/// }
/// ```
///
/// `Query` also implements `IntoIterator` for `&mut Query`, so `for item in &mut q` works too,
/// identically to `for item in q.iter()`.
///
/// Include `hecs::Entity` in `Q` (e.g. `Query<(Entity, &Position)>`) if you need the entity id
/// back alongside its components — it's a query term like any other, not a separate mechanism.
///
/// # Narrowing by component (`with`/`without`)
///
/// [`with`](Self::with)/[`without`](Self::without) narrow which entities match, without adding
/// to (or needing) the yielded item type, and — unlike calling straight through to `hecs` —
/// chain: each returns another `Query`, so `.with::<&Enemy>().without::<&Dead>().iter()` keeps
/// every method on this page available at each step, no `hecs::With`/`hecs::Without` naming
/// required on your end.
///
/// # Single-entity lookups
///
/// Use [`Query::get`] to fetch components for one known `Entity` without scanning the whole
/// result set, and [`Query::single`]/[`Query::get_single`] when you expect exactly one match
/// (e.g. "the player", "the active camera").
pub struct Query<'a, Q: hecs::Query> {
    world: &'a hecs::World,
    borrow: hecs::QueryBorrow<'a, Q>,
    /// Scratch storage for [`get`](Self::get) — a fresh one-shot lookup is built into this slot
    /// on every call (dropping whatever was there from the last call) so the item it returns
    /// can borrow from `self` (stable, caller-controlled) instead of a temporary that would be
    /// gone by the time the caller could use it.
    scratch: Option<hecs::QueryOne<'a, Q>>,
}

impl<'q, Q: hecs::Query> IntoIterator for &'q mut Query<'_, Q> {
    type Item = Q::Item<'q>;
    type IntoIter = hecs::QueryIter<'q, Q>;

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.borrow).into_iter()
    }
}

impl<'a, Q: hecs::Query> Query<'a, Q> {
    /// Iterate every entity matching this query. An ordinary `Iterator` — `.filter(...)`,
    /// `.map(...)`, `.count()`, `.collect()`, and every other standard adapter work directly on
    /// the result, no `hecs` types involved.
    pub fn iter(&mut self) -> impl Iterator<Item = Q::Item<'_>> {
        self.borrow.iter()
    }

    /// Look up a single entity's components for this query. `None` if the entity doesn't exist
    /// or doesn't match `Q`.
    pub fn get(&mut self, entity: hecs::Entity) -> Option<Q::Item<'_>> {
        self.scratch = Some(self.world.query_one::<Q>(entity));
        self.scratch.as_mut().unwrap().get().ok()
    }

    /// Narrow this query to only entities that ALSO have component(s) `R`, without `R` itself
    /// being part of the yielded items. Consumes `self` and returns another `Query` — chain
    /// further `.with`/`.without`, or call `.iter`/`.get`/`.single` directly on the result.
    pub fn with<R: hecs::Query>(self) -> Query<'a, hecs::With<Q, R>> {
        Query { world: self.world, borrow: self.borrow.with::<R>(), scratch: None }
    }

    /// Narrow this query to only entities that do NOT have component(s) `R`. Same shape as
    /// [`with`](Self::with).
    pub fn without<R: hecs::Query>(self) -> Query<'a, hecs::Without<Q, R>> {
        Query { world: self.world, borrow: self.borrow.without::<R>(), scratch: None }
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
            type_id: std::any::TypeId::of::<T>(),
            present: |world, resources| resources.has_resource::<T>(world),
            hint: None,
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
            type_id: std::any::TypeId::of::<T>(),
            present: |world, resources| resources.has_resource::<T>(world),
            hint: None,
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
            world,
            borrow: world.query::<Q>(),
            scratch: None,
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

    /// This system's identity for ordering purposes — the [`TypeId`](std::any::TypeId)
    /// of the function/closure it wraps. Automatic: every distinct function
    /// or closure has a distinct type, so no manual labeling is needed to
    /// make a system a valid target for another system's
    /// [`after`](SystemOrderingExt::after)/[`before`](SystemOrderingExt::before).
    /// Defaults to `Self`'s own `TypeId`; [`FunctionSystem`]/[`OnceFunctionSystem`]
    /// override it with the wrapped function's `TypeId` instead of the
    /// wrapper's, so ordering constraints referencing the bare function match.
    fn ordering_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }

    /// Other systems in the same stage that must run before this one. Set
    /// via [`SystemOrderingExt::after`]. A referenced system that isn't
    /// registered in the same stage is silently ignored.
    fn after_ids(&self) -> &[std::any::TypeId] {
        &[]
    }

    /// Other systems in the same stage that must run after this one. Set
    /// via [`SystemOrderingExt::before`]. A referenced system that isn't
    /// registered in the same stage is silently ignored.
    fn before_ids(&self) -> &[std::any::TypeId] {
        &[]
    }
}

/// Wraps a [`System`] with ordering constraints relative to other systems in
/// the same stage, added via [`SystemOrderingExt`].
///
/// Constraints only take effect within the stage the system is registered
/// to — there's no cross-stage ordering, since stage order is already fixed
/// by [`SystemStage`](crate::app::SystemStage). [`App::build`](crate::app::App::build)
/// topologically sorts each stage's systems by these constraints, breaking
/// ties by registration order, and panics if constraints form a cycle.
pub struct Labeled<S: System> {
    inner: S,
    after: Vec<std::any::TypeId>,
    before: Vec<std::any::TypeId>,
}

impl<S: System> Labeled<S> {
    /// Require that `system` runs before this one, within the same stage.
    /// Chainable — call multiple times to depend on multiple systems.
    pub fn after<F: 'static, Marker>(mut self, system: F) -> Self
    where
        F: IntoSystem<Marker>,
    {
        let _ = system;
        self.after.push(std::any::TypeId::of::<F>());
        self
    }

    /// Require that `system` runs after this one, within the same stage.
    /// Chainable — call multiple times to constrain multiple systems.
    pub fn before<F: 'static, Marker>(mut self, system: F) -> Self
    where
        F: IntoSystem<Marker>,
    {
        let _ = system;
        self.before.push(std::any::TypeId::of::<F>());
        self
    }
}

impl<S: System> System for Labeled<S> {
    fn run(&mut self, world: &hecs::World, resources: &Resources) {
        self.inner.run(world, resources)
    }

    fn requires(&self) -> Vec<RequiredResource> {
        self.inner.requires()
    }

    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn ordering_id(&self) -> std::any::TypeId {
        self.inner.ordering_id()
    }

    fn after_ids(&self) -> &[std::any::TypeId] {
        &self.after
    }

    fn before_ids(&self) -> &[std::any::TypeId] {
        &self.before
    }
}

impl<S: System> IntoSystem<()> for Labeled<S> {
    type System = Self;

    fn into_system(self) -> Self::System {
        self
    }
}

/// Adds [`.after()`](SystemOrderingExt::after)/[`.before()`](SystemOrderingExt::before)
/// to any system, for declaring run-order constraints relative to other
/// systems in the same stage — referenced directly by their function/closure,
/// no string labels needed.
///
/// ```ignore
/// app.add_system(SystemStage::Update, physics_step);
/// app.add_system(SystemStage::Update, apply_damage.after(physics_step));
/// ```
pub trait SystemOrderingExt<Marker>: IntoSystem<Marker> + Sized {
    /// Require that `system` runs before this one, within the same stage.
    fn after<F: 'static, Marker2>(self, system: F) -> Labeled<Self::System>
    where
        F: IntoSystem<Marker2>,
    {
        let _ = system;
        Labeled {
            inner: self.into_system(),
            after: vec![std::any::TypeId::of::<F>()],
            before: Vec::new(),
        }
    }

    /// Require that `system` runs after this one, within the same stage.
    fn before<F: 'static, Marker2>(self, system: F) -> Labeled<Self::System>
    where
        F: IntoSystem<Marker2>,
    {
        let _ = system;
        Labeled {
            inner: self.into_system(),
            after: Vec::new(),
            before: vec![std::any::TypeId::of::<F>()],
        }
    }
}

impl<T, Marker> SystemOrderingExt<Marker> for T where T: IntoSystem<Marker> {}

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

            fn ordering_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<T>()
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

/// Type-erased wrapper produced by [`OnceExt::once`]. Runs `func` every time
/// it's invoked until `func` returns `Some(())`, at which point it's
/// permanently retired — every subsequent invocation (and requirement check)
/// is a no-op. The "have I already succeeded" bookkeeping lives entirely in
/// `done`, hidden inside this wrapper; the wrapped function itself just
/// returns `None` ("not ready, call me again") or `Some(())` ("done").
pub struct OnceFunctionSystem<F, Marker, State = ()> {
    func: F,
    state: State,
    done: bool,
    _marker: std::marker::PhantomData<Marker>,
}

/// Adds [`.once()`](OnceExt::once) to a function/closure whose parameters
/// are valid [`SystemParam`]s and whose return type is `Option<()>`,
/// registering it as a system that runs on every tick of whichever stage
/// it's added to until it returns `Some(())`, then never runs again.
///
/// This replaces manually tracking a "have I already done this" flag with
/// a `Local<bool>`: return `None` from the function to mean "not ready,
/// try again next tick" and `Some(())` to mean "done, retire me".
///
/// ```ignore
/// fn setup(mut commands: Commands, pbr: Option<Res<PBR>>) -> Option<()> {
///     let pbr = pbr?;
///     if pbr.cubemap_material_inst == RawAssetHandle::default() {
///         return None; // not ready yet — try again next tick
///     }
///     commands.spawn(/* ... */);
///     Some(()) // done — never runs again
/// }
///
/// app.add_system(SystemStage::PreUpdate, setup.once());
/// ```
pub trait OnceExt<Marker> {
    type System: System;
    fn once(self) -> Self::System;
}

macro_rules! impl_once_system {
    ($($param:ident),*) => {
        impl<T, $($param),*> OnceExt<($($param,)*)> for T
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Option<()> + 'static,
            for<'a> &'a mut T: FnMut($($param),*) -> Option<()>,
            $($param: SystemParam + 'static),*
        {
            type System = OnceFunctionSystem<T, ($($param,)*), ($($param::State,)*)>;

            fn once(self) -> Self::System {
                OnceFunctionSystem {
                    func: self,
                    state: Default::default(),
                    done: false,
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<T, $($param),*> IntoSystem<($($param,)*)> for OnceFunctionSystem<T, ($($param,)*), ($($param::State,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Option<()> + 'static,
            $($param: SystemParam + 'static),*
        {
            type System = Self;

            fn into_system(self) -> Self::System {
                self
            }
        }

        impl<T, $($param),*> System for OnceFunctionSystem<T, ($($param,)*), ($($param::State,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Option<()> + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _world: &hecs::World, _resources: &Resources) {
                if self.done {
                    return;
                }
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                let result = (self.func)($($param::fetch($param, _world, _resources)),*);
                if result.is_some() {
                    self.done = true;
                }
            }

            fn requires(&self) -> Vec<RequiredResource> {
                if self.done {
                    return Vec::new();
                }
                let mut _v = Vec::new();
                $(_v.extend($param::requires());)*
                _v
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<T>()
            }

            fn ordering_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<T>()
            }
        }
    };
}

impl_once_system!();
impl_once_system!(A);
impl_once_system!(A, B);
impl_once_system!(A, B, C);
impl_once_system!(A, B, C, D);
impl_once_system!(A, B, C, D, E);
impl_once_system!(A, B, C, D, E, F);
impl_once_system!(A, B, C, D, E, F, G);
impl_once_system!(A, B, C, D, E, F, G, H);
impl_once_system!(A, B, C, D, E, F, G, H, I);
impl_once_system!(A, B, C, D, E, F, G, H, I, J);
impl_once_system!(A, B, C, D, E, F, G, H, I, J, K);
impl_once_system!(A, B, C, D, E, F, G, H, I, J, K, L);

/// Type-erased wrapper produced by [`AsyncExt::detach`]. See that method's
/// docs for the fire-and-forget semantics.
pub struct DetachedFunctionSystem<F, Marker, State = ()> {
    func: F,
    state: State,
    _marker: std::marker::PhantomData<Marker>,
}

/// Adds [`.detach()`](AsyncExt::detach) to a function/closure whose
/// parameters are valid [`SystemParam`]s and which returns a
/// `Future<Output = ()> + Send + 'static`, registering it as a system.
///
/// Each tick, the wrapped function is called synchronously like any other
/// system — its `SystemParam`s (`Res`, `Query`, ...) are fetched and
/// borrowed exactly as usual — but instead of doing work directly, it
/// builds and returns a future (typically an `async move { .. }` block that
/// has cloned or copied out whatever owned data it needs from those
/// borrows). The scheduler then hands that future to
/// [`BackgroundTasks::spawn_async`](crate::threading::BackgroundTasks::spawn_async)
/// and moves on immediately — the future runs to completion on a worker
/// thread, off the main loop, with no access to the `World`/`Resources`
/// (which is exactly why it has to be `'static`: nothing borrowed from this
/// tick is valid once the future outlives it).
///
/// A real `async fn` can't be used directly as the wrapped function here:
/// its returned future borrows every one of its parameters by construction,
/// so it's never `'static` on its own. Extract the owned pieces you need in
/// the ordinary (synchronous) function body, then move only those into the
/// `async move` block you return.
///
/// Fire-and-forget: nothing delivers the future's result back
/// automatically, and a system that unconditionally detaches a new future
/// every tick will spawn a new one every tick. If you need the result, or
/// want to send only once, call [`BackgroundTasks::spawn_async`](crate::threading::BackgroundTasks::spawn_async) yourself
/// inside an ordinary system (guarding with [`Local<bool>`](Local) or
/// [`OnceExt::once`] as needed) and poll the returned
/// [`TaskHandle`](crate::threading::TaskHandle) — same pattern already used
/// for the async GPU backend init.
///
/// ```ignore
/// fn load_level(tasks: Res<BackgroundTasks>) -> impl Future<Output = ()> + Send + 'static {
///     let tasks = tasks.clone();
///     async move {
///         let bytes = std::fs::read("level.bin").unwrap();
///         // ... process `bytes`, maybe tasks.spawn_blocking(...) more work ...
///     }
/// }
///
/// app.add_system(SystemStage::Update, load_level.detach());
/// ```
pub trait AsyncExt<Marker> {
    type System: System;
    fn detach(self) -> Self::System;
}

macro_rules! impl_async_system {
    ($($param:ident),*) => {
        impl<T, Fut, $($param),*> AsyncExt<($($param,)*)> for T
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Fut + 'static,
            for<'a> &'a mut T: FnMut($($param),*) -> Fut,
            Fut: SpawnableFuture<()>,
            $($param: SystemParam + 'static),*
        {
            type System = DetachedFunctionSystem<T, ($($param,)*), ($($param::State,)*)>;

            fn detach(self) -> Self::System {
                DetachedFunctionSystem {
                    func: self,
                    state: Default::default(),
                    _marker: std::marker::PhantomData,
                }
            }
        }

        impl<T, Fut, $($param),*> IntoSystem<($($param,)*)> for DetachedFunctionSystem<T, ($($param,)*), ($($param::State,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Fut + 'static,
            Fut: SpawnableFuture<()>,
            $($param: SystemParam + 'static),*
        {
            type System = Self;

            fn into_system(self) -> Self::System {
                self
            }
        }

        impl<T, Fut, $($param),*> System for DetachedFunctionSystem<T, ($($param,)*), ($($param::State,)*)>
        where
            T: for<'a> FnMut($($param::Item<'a>),*) -> Fut + 'static,
            Fut: SpawnableFuture<()>,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _world: &hecs::World, _resources: &Resources) {
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                let future = (self.func)($($param::fetch($param, _world, _resources)),*);
                let tasks = _resources.get_resource::<crate::threading::BackgroundTasks>(_world);
                let _ = tasks.spawn_async(future);
            }

            fn requires(&self) -> Vec<RequiredResource> {
                let mut _v = vec![RequiredResource {
                    name: std::any::type_name::<crate::threading::BackgroundTasks>(),
                    type_id: std::any::TypeId::of::<crate::threading::BackgroundTasks>(),
                    present: |world, resources| resources.has_resource::<crate::threading::BackgroundTasks>(world),
                    hint: Some(
                        "`.detach()` drives its future through `BackgroundTasks` — register \
                         `app.add_plugin(BackgroundTasksPlugin::new(worker_count))` before this system runs.",
                    ),
                }];
                $(_v.extend($param::requires());)*
                _v
            }

            fn name(&self) -> &'static str {
                std::any::type_name::<T>()
            }

            fn ordering_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<T>()
            }
        }
    };
}

impl_async_system!();
impl_async_system!(A);
impl_async_system!(A, B);
impl_async_system!(A, B, C);
impl_async_system!(A, B, C, D);
impl_async_system!(A, B, C, D, E);
impl_async_system!(A, B, C, D, E, F);
impl_async_system!(A, B, C, D, E, F, G);
impl_async_system!(A, B, C, D, E, F, G, H);
impl_async_system!(A, B, C, D, E, F, G, H, I);
impl_async_system!(A, B, C, D, E, F, G, H, I, J);
impl_async_system!(A, B, C, D, E, F, G, H, I, J, K);
impl_async_system!(A, B, C, D, E, F, G, H, I, J, K, L);

#[cfg(test)]
mod tests {
    use super::*;

    struct Health(i32);
    struct Enemy;
    struct Dead;

    fn make_query<Q: hecs::Query>(world: &hecs::World) -> Query<'_, Q> {
        Query { world, borrow: world.query::<Q>(), scratch: None }
    }

    #[test]
    fn iter_yields_every_matching_entity() {
        let mut world = hecs::World::new();
        world.spawn((Health(10),));
        world.spawn((Health(20),));

        let mut query = make_query::<&Health>(&world);
        let mut totals: Vec<i32> = query.iter().map(|h| h.0).collect();
        totals.sort();
        assert_eq!(totals, vec![10, 20]);
    }

    #[test]
    fn iter_composes_with_standard_iterator_adapters() {
        // The whole point of returning a plain `impl Iterator` from `iter()` — `.filter(...)`
        // (a value-level predicate) needs nothing beyond what `std::iter` already provides.
        let mut world = hecs::World::new();
        world.spawn((Health(5),));
        world.spawn((Health(50),));

        let mut query = make_query::<&Health>(&world);
        let low_health_count = query.iter().filter(|h| h.0 < 10).count();
        assert_eq!(low_health_count, 1);
    }

    #[test]
    fn get_returns_some_for_a_matching_entity_and_none_otherwise() {
        let mut world = hecs::World::new();
        let matching = world.spawn((Health(7),));
        let non_matching = world.spawn(()); // no Health

        let mut query = make_query::<&Health>(&world);
        assert_eq!(query.get(matching).map(|h| h.0), Some(7));
        assert!(query.get(non_matching).is_none());
    }

    #[test]
    fn get_can_be_called_more_than_once_on_the_same_query() {
        // Regression check for the `scratch` slot: a second `.get()` call must not panic or
        // reuse a stale `hecs::QueryOne` (which panics if `.get()` is called on it twice).
        let mut world = hecs::World::new();
        let a = world.spawn((Health(1),));
        let b = world.spawn((Health(2),));

        let mut query = make_query::<&Health>(&world);
        assert_eq!(query.get(a).map(|h| h.0), Some(1));
        assert_eq!(query.get(b).map(|h| h.0), Some(2));
    }

    #[test]
    fn with_and_without_chain_and_narrow_by_component_presence() {
        let mut world = hecs::World::new();
        let alive_enemy = world.spawn((Health(1), Enemy));
        world.spawn((Health(1), Enemy, Dead));
        world.spawn((Health(1),));

        let query = make_query::<&Health>(&world);
        // Chained: still the engine's own `Query`, not a raw `hecs::QueryBorrow`/`With`/
        // `Without` — `.get`/`.iter` remain available after narrowing.
        let mut narrowed = query.with::<&Enemy>().without::<&Dead>();

        assert_eq!(narrowed.iter().count(), 1);
        assert!(narrowed.get(alive_enemy).is_some());
    }

    #[test]
    fn single_panics_on_zero_or_multiple_matches_get_single_does_not() {
        let mut world = hecs::World::new();

        assert!(make_query::<&Health>(&world).get_single().is_none());

        world.spawn((Health(1),));
        assert_eq!(make_query::<&Health>(&world).single().0, 1);

        world.spawn((Health(2),));
        assert!(make_query::<&Health>(&world).get_single().is_none());
    }
}
