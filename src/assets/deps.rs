use crate::{ecs::resources::Resources, ecs::system::Res};

/// A set of resources required by an [`Asset`](crate::assets::upload::Asset)
/// implementation before it can be uploaded to the backend.
///
/// Implement this trait for `()` (no deps), a single `Res<T>`, or a tuple of
/// `Res<T>` values (up to 8). The sync system calls [`try_gather`] each tick
/// and skips the upload if any dependency is not yet present.
pub trait Dependencies<'a>: Sized {
    /// Attempt to gather all required resources from the world.
    ///
    /// Returns `None` if any dependency is missing, deferring the upload to
    /// the next tick.
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self>;
}

impl<'a> Dependencies<'a> for () {
    fn try_gather(_world: &'a hecs::World, _resources: &'a Resources) -> Option<Self> {
        Some(())
    }
}

impl<'a, A: 'static + Send + Sync> Dependencies<'a> for Res<'a, A> {
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self> {
        if !resources.has_resource::<A>(world) {
            return None;
        }
        Some(Res {
            data: resources.get_resource(world),
        })
    }
}

/// Implements [`Dependencies`] for a tuple of `Res<'a, T>` values.
///
/// Invoked once per arity (2 through 8); each invocation lists the generic
/// type parameters for that tuple's elements.
macro_rules! impl_dependencies_tuple {
    ($($T:ident),+ $(,)?) => {
        impl<'a, $($T),+> Dependencies<'a> for ($(Res<'a, $T>),+,)
        where
            $($T: 'static + Send + Sync),+
        {
            fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self> {
                if $(!resources.has_resource::<$T>(world))||+ {
                    return None;
                }
                Some((
                    $(
                        Res::<$T> {
                            data: resources.get_resource(world),
                        }
                    ),+,
                ))
            }
        }
    };
}

impl_dependencies_tuple!(A, B);
impl_dependencies_tuple!(A, B, C);
impl_dependencies_tuple!(A, B, C, D);
impl_dependencies_tuple!(A, B, C, D, E);
impl_dependencies_tuple!(A, B, C, D, E, F);
impl_dependencies_tuple!(A, B, C, D, E, F, G);
impl_dependencies_tuple!(A, B, C, D, E, F, G, H);
