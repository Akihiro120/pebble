use crate::ecs::resources::Resources;

pub trait SystemParam {
    type Item<'a>;
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

pub trait System: 'static {
    fn run(&mut self, world: &hecs::World, resources: &Resources);
}

pub struct FunctionSystem<F, Marker, State = ()> {
    pub func: F,
    state: State,
    _marker: std::marker::PhantomData<Marker>,
}

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
