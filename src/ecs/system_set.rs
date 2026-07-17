use crate::ecs::system::{IntoSystem, System};

/// Converts a tuple of systems into a `Vec<Box<dyn System>>` so they can be
/// registered together with [`App::add_systems`](crate::app::App::add_systems).
pub trait IntoSystemSet<M> {
    fn into_system_set(self) -> Vec<Box<dyn System>>;
}

macro_rules! impl_into_system_set {
    ($(($sys:ident, $marker:ident)),+) => {
        impl<$($sys, $marker),+> IntoSystemSet<($($marker,)+)> for ($($sys,)+)
        where
            $($sys: IntoSystem<$marker> + 'static,)+
        {
            fn into_system_set(self) -> Vec<Box<dyn System>> {
                #[allow(non_snake_case)]
                let ($($sys,)+) = self;
                vec![$(Box::new($sys.into_system()) as Box<dyn System>),+]
            }
        }
    };
}

impl_into_system_set!((A, MA));
impl_into_system_set!((A, MA), (B, MB));
impl_into_system_set!((A, MA), (B, MB), (C, MC));
impl_into_system_set!((A, MA), (B, MB), (C, MC), (D, MD));
impl_into_system_set!((A, MA), (B, MB), (C, MC), (D, MD), (E, ME));
impl_into_system_set!((A, MA), (B, MB), (C, MC), (D, MD), (E, ME), (G, MG));
impl_into_system_set!(
    (A, MA),
    (B, MB),
    (C, MC),
    (D, MD),
    (E, ME),
    (G, MG),
    (H, MH)
);
