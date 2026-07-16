use crate::{ecs::resources::Resources, ecs::system::Res};

pub trait Dependencies<'a>: Sized {
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

impl<'a, A, B> Dependencies<'a> for (Res<'a, A>, Res<'a, B>)
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self> {
        if !resources.has_resource::<A>(world) || !resources.has_resource::<B>(world) {
            return None;
        }

        Some((
            Res {
                data: resources.get_resource(world),
            },
            Res {
                data: resources.get_resource(world),
            },
        ))
    }
}
