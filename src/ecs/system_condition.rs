use crate::ecs::{
    resources::Resources,
    system::{IntoSystem, System},
    system_set::IntoSystemSet,
};

/// A predicate checked before a system runs. If `should_run` returns
/// `false`, the wrapped system's body is skipped entirely for that tick —
/// its `SystemParam`s are never fetched.
///
/// Re-checked every tick. This is deliberate: a resource that exists now
/// is not guaranteed to exist forever (e.g. if something explicitly removes
/// it later), so conditions should not assume "ready once" means "ready
/// forever".
pub trait RunCondition: 'static {
    fn should_run(world: &hecs::World, resources: &Resources) -> bool;
}

/// Runs only while resource `T` exists.
pub struct ResourceExists<T>(std::marker::PhantomData<T>);
impl<T: 'static + Send + Sync> RunCondition for ResourceExists<T> {
    fn should_run(world: &hecs::World, resources: &Resources) -> bool {
        resources.has_resource::<T>(world)
    }
}

/// Runs only while both `A` and `B` would run.
pub struct And<A, B>(std::marker::PhantomData<(A, B)>);
impl<A: RunCondition, B: RunCondition> RunCondition for And<A, B> {
    fn should_run(world: &hecs::World, resources: &Resources) -> bool {
        A::should_run(world, resources) && B::should_run(world, resources)
    }
}

/// Runs while either `A` or `B` would run.
pub struct Or<A, B>(std::marker::PhantomData<(A, B)>);
impl<A: RunCondition, B: RunCondition> RunCondition for Or<A, B> {
    fn should_run(world: &hecs::World, resources: &Resources) -> bool {
        A::should_run(world, resources) || B::should_run(world, resources)
    }
}

/// Wraps a [`System`], skipping it for a tick whenever `C::should_run`
/// returns `false`.
pub struct Conditional<S, C> {
    inner: S,
    _marker: std::marker::PhantomData<C>,
}

impl<S: System, C: RunCondition> System for Conditional<S, C> {
    fn run(&mut self, world: &hecs::World, resources: &Resources) {
        if C::should_run(world, resources) {
            self.inner.run(world, resources);
        }
    }

    fn requires(&self) -> Vec<crate::ecs::system::RequiredResource> {
        self.inner.requires()
    }
}

/// Adds [`.run_if`](RunIfExt::run_if) to anything convertible into a
/// [`System`], gating it on a [`RunCondition`].
pub trait RunIfExt<Marker>: IntoSystem<Marker> + Sized {
    fn run_if<C: RunCondition>(self) -> RunIfSystem<Self, Marker, C> {
        RunIfSystem {
            inner: self,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<Marker, T: IntoSystem<Marker>> RunIfExt<Marker> for T {}

pub struct RunIfSystem<T, Marker, C> {
    inner: T,
    _marker: std::marker::PhantomData<(Marker, C)>,
}

impl<T, Marker, C> IntoSystem<Marker> for RunIfSystem<T, Marker, C>
where
    T: IntoSystem<Marker>,
    C: RunCondition,
{
    type System = Conditional<T::System, C>;

    fn into_system(self) -> Self::System {
        Conditional {
            inner: self.inner.into_system(),
            _marker: std::marker::PhantomData,
        }
    }
}

/// Adds [`.run_if`](SystemSetRunIfExt::run_if) to an entire
/// [`IntoSystemSet`] tuple at once, applying the same condition to every
/// system in it.
pub trait SystemSetRunIfExt<M>: IntoSystemSet<M> + Sized {
    fn run_if<C: RunCondition>(self) -> ConditionalSet<Self, M, C> {
        ConditionalSet {
            inner: self,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M, T: IntoSystemSet<M>> SystemSetRunIfExt<M> for T {}

pub struct ConditionalSet<T, M, C> {
    inner: T,
    _marker: std::marker::PhantomData<(M, C)>,
}

/// Wraps an already-boxed, type-erased [`System`] with a [`RunCondition`].
/// Used internally by [`ConditionalSet`], since by the time a system set is
/// flattened into `Vec<Box<dyn System>>`, individual marker types are gone.
struct BoxedConditional<C> {
    inner: Box<dyn System>,
    _marker: std::marker::PhantomData<C>,
}

impl<C: RunCondition> System for BoxedConditional<C> {
    fn run(&mut self, world: &hecs::World, resources: &Resources) {
        if C::should_run(world, resources) {
            self.inner.run(world, resources);
        }
    }

    fn requires(&self) -> Vec<crate::ecs::system::RequiredResource> {
        self.inner.requires()
    }
}

impl<T, M, C> IntoSystemSet<M> for ConditionalSet<T, M, C>
where
    T: IntoSystemSet<M>,
    C: RunCondition,
{
    fn into_system_set(self) -> Vec<Box<dyn System>> {
        self.inner
            .into_system_set()
            .into_iter()
            .map(|system| {
                Box::new(BoxedConditional::<C> {
                    inner: system,
                    _marker: std::marker::PhantomData,
                }) as Box<dyn System>
            })
            .collect()
    }
}
