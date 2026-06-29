pub struct Res<T: 'static> {
    _marker: std::marker::PhantomData<T>,
}

pub struct Query<Q: hecs::Query> {
    _marker: std::marker::PhantomData<Q>,
}

trait SystemParam {
    type Item<'a>;
    fn fetch<'a>(world: &'a hecs::World) -> Self::Item<'a>;
}

// impl<T> SystemParam for Res<T>
// where
//     T: 'static,
// {
//     type Item = Res<T>;
//
//     fn fetch(world: &hecs::World) -> Self::Item {}
// }

impl<Q> SystemParam for Query<Q>
where
    Q: hecs::Query,
{
    type Item<'a> = hecs::QueryBorrow<'a, Q>;

    fn fetch<'a>(world: &'a hecs::World) -> Self::Item<'a> {
        world.query::<Q>()
    }
}

pub trait System: 'static {
    fn run(&mut self, world: &hecs::World);
}

pub struct FunctionSystem<F, Marker> {
    pub func: F,
    _marker: std::marker::PhantomData<Marker>,
}

pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
}

macro_rules! impl_system {
    ($($param:ident),*) => {
        impl<F, $($param),*> System for FunctionSystem<F, ($($param,)*)>
        where
            F: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            fn run(&mut self, world: &hecs::World) {
                (self.func)($($param::fetch(world)),*);
            }
        }

        impl<F, $($param),*> IntoSystem<($($param,)*)> for F
        where
            F: for<'a> FnMut($($param::Item<'a>),*) + 'static,
            $($param: SystemParam + 'static),*
        {
            type System = FunctionSystem<F, ($($param,)*)>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    func: self,
                    _marker: std::marker::PhantomData,
                }
            }
        }
    };
}

impl_system!();
impl_system!(A);
impl_system!(A, B, C);
impl_system!(A, B, C, D);
impl_system!(A, B, C, D, E);
impl_system!(A, B, C, D, E, G);
impl_system!(A, B, C, D, E, G, H);
