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

pub trait SystemParamFunction<Params>: 'static {
    type State: Default + 'static;

    fn run_system(&mut self, world: &hecs::World, resources: &Resources, state: &mut Self::State);
}

macro_rules! impl_system_param_function {
    ($($P:ident),*) => {
        #[allow(non_snake_case, unused_variables)]
        impl<Func, $($P: SystemParam),*> SystemParamFunction<($($P,)*)> for Func
        where
            Func: FnMut($($P),*) + for<'w> FnMut($($P::Item<'w>),*) + 'static,
        {
            type State = ($($P::State,)*);

            fn run_system(&mut self, world: &hecs::World, resources: &Resources, state: &mut Self::State) {
                let ($($P,)*) = state;
                $(
                    let $P = $P::fetch(world, resources, $P);
                )*
                self($($P),*);
            }
        }
    };
}

impl_system_param_function!();
impl_system_param_function!(P1);
impl_system_param_function!(P1, P2);
impl_system_param_function!(P1, P2, P3);
impl_system_param_function!(P1, P2, P3, P4);
impl_system_param_function!(P1, P2, P3, P4, P5);
impl_system_param_function!(P1, P2, P3, P4, P5, P6);
impl_system_param_function!(P1, P2, P3, P4, P5, P6, P7);
impl_system_param_function!(P1, P2, P3, P4, P5, P6, P7, P8);

pub trait StoredSystem {
    fn run(&mut self, world: &hecs::World, resources: &Resources);
}

struct SystemContainer<F: SystemParamFunction<Params>, Params> {
    f: F,
    state: F::State,
    marker: std::marker::PhantomData<fn() -> Params>,
}

impl<F, Params> StoredSystem for SystemContainer<F, Params>
where
    F: SystemParamFunction<Params>,
    Params: 'static,
{
    fn run(&mut self, world: &hecs::World, resources: &Resources) {
        self.f.run_system(world, resources, &mut self.state);
    }
}

pub trait IntoSystem<Params> {
    fn into_system(self) -> Box<dyn StoredSystem>;
}

impl<F, Params> IntoSystem<Params> for F
where
    F: SystemParamFunction<Params> + 'static,
    Params: 'static,
{
    fn into_system(self) -> Box<dyn StoredSystem> {
        Box::new(SystemContainer {
            state: F::State::default(),
            f: self,
            marker: std::marker::PhantomData,
        })
    }
}
