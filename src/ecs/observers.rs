use crate::ecs::{resources::Resources, system_param::SystemParam};

pub struct Trigger<'a, E> {
    event: &'a E,
}

impl<'a, E> std::ops::Deref for Trigger<'a, E> {
    type Target = E;

    fn deref(&self) -> &E {
        self.event
    }
}

pub trait ObserverSystem<E>: 'static {
    fn run(&mut self, event: &E, world: &hecs::World, resources: &Resources);
}

pub struct FunctionObserver<F, E, Marker, State = ()> {
    func: F,
    state: State,
    _marker: std::marker::PhantomData<(E, Marker)>,
}

pub trait IntoObserverSystem<E, Marker> {
    type System: ObserverSystem<E>;

    fn into_observer_system(self) -> Self::System;
}

macro_rules! impl_observer_system {
    ($($param:ident),*) => {
        impl<T, Evt: 'static, $($param),*> IntoObserverSystem<Evt, ($($param,)*)> for T
        where
            T: FnMut(Trigger<Evt>, $($param),*) + for<'a> FnMut(Trigger<'a, Evt>, $($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            type System = FunctionObserver<T, Evt, ($($param,)*), ($($param::State,)*)>;
            fn into_observer_system(self) -> Self::System {
                FunctionObserver {
                    func: self,
                    state: Default::default(),
                    _marker: std::marker::PhantomData,
                }
            }
        }
        impl<T, Evt: 'static, $($param),*> ObserverSystem<Evt> for FunctionObserver<T, Evt, ($($param,)*), ($($param::State,)*)>
        where
            T: FnMut(Trigger<Evt>, $($param),*) + for<'a> FnMut(Trigger<'a, Evt>, $($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, _event: &Evt, _world: &hecs::World, _resources: &Resources) {
                let trigger = Trigger { event: _event };
                #[allow(non_snake_case)]
                let ($($param,)*) = &mut self.state;
                (self.func)(trigger, $($param::fetch(_world, _resources, $param)),*);
            }
        }
    };
}

impl_observer_system!();
impl_observer_system!(A);
impl_observer_system!(A, B);
impl_observer_system!(A, B, C);
impl_observer_system!(A, B, C, D);
impl_observer_system!(A, B, C, D, E);
impl_observer_system!(A, B, C, D, E, F);
impl_observer_system!(A, B, C, D, E, F, G);
impl_observer_system!(A, B, C, D, E, F, G, H);
impl_observer_system!(A, B, C, D, E, F, G, H, I);
impl_observer_system!(A, B, C, D, E, F, G, H, I, J);
impl_observer_system!(A, B, C, D, E, F, G, H, I, J, K);
impl_observer_system!(A, B, C, D, E, F, G, H, I, J, K, L);

pub(crate) struct Observers<E>(pub(crate) Vec<Box<dyn ObserverSystem<E>>>);

impl<E> Default for Observers<E> {
    fn default() -> Self {
        Self(Vec::new())
    }
}
