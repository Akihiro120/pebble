use crate::ecs::resources::{Read, Resources};

pub trait Dependencies<'a>: Sized {
    fn try_gather(resources: &'a Resources) -> Option<Self>;
}

impl<'a> Dependencies<'a> for () {
    fn try_gather(_resources: &'a Resources) -> Option<Self> {
        Some(())
    }
}

impl<'a, A: 'static> Dependencies<'a> for Read<'a, A> {
    fn try_gather(resources: &'a Resources) -> Option<Self> {
        if !resources.contains::<A>() {
            return None;
        }
        Some(Read {
            inner: resources.get::<A>(),
        })
    }
}

macro_rules! impl_dependencies_tuple {
    ($($T:ident),+ $(,)?) => {
        impl<'a, $($T: 'static),+> Dependencies<'a> for ($(Read<'a, $T>,)+) {
            fn try_gather(resources: &'a Resources) -> Option<Self> {
                if $(!resources.contains::<$T>())||+ {
                    return None;
                }
                Some((
                    $(Read::<$T> { inner: resources.get::<$T>() },)+
                ))
            }
        }
    };
}

impl_dependencies_tuple!(A, B);
impl_dependencies_tuple!(A, B, C);
impl_dependencies_tuple!(A, B, C, D);
impl_dependencies_tuple!(A, B, C, D, E);
