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
