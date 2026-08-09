use crate::ecs::{
    resources::Resources,
    system::{IntoSystem, System},
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

    // Deliberately does NOT forward `requires()`: wrapping a system in
    // `.run_if()` means its author has taken over the "is it safe to run"
    // question themselves (often via `ResourceExists<T>` for the very
    // resource the body needs) — App shouldn't second-guess that with its
    // own pre-flight check on top.
    fn name(&self) -> &'static str {
        self.inner.name()
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
