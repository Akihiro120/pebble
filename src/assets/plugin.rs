use crate::{
    app::App,
    assets::{deps::Dependencies, handle::Handle, storage::Assets, upload::Asset},
    ecs::{
        plugin::Plugin,
        resources::{Read, Resources, Write},
        system::SystemStage,
    },
};

pub struct AssetPlugin<B, T: Asset<B>> {
    _marker: std::marker::PhantomData<(B, T)>,
}

impl<B, T: Asset<B>> AssetPlugin<B, T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B, T> Plugin for AssetPlugin<B, T>
where
    B: 'static + Send + Sync,
    T: Asset<B> + 'static,
{
    fn build(self, app: App) -> App {
        app.insert_resource(Assets::<T>::new())
            .add_system(SystemStage::AssetSync, sync_assets::<B, T>)
    }
}

fn sync_assets<B, T>(mut assets: Write<Assets<T>>, backend: Option<Read<B>>, resources: &Resources)
where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    let Some(backend) = backend else {
        return;
    };
    let Some(deps) = T::Deps::try_gather(resources) else {
        return;
    };

    let dirty = assets.take_dirty();
    let mut still_pending = Vec::new();

    for raw in dirty {
        let handle = Handle::<T>::new(raw);
        let Some(source) = assets.get_source(handle) else {
            continue;
        };

        match source.upload(&backend, &deps) {
            None => still_pending.push(raw),
            Some(processed) => assets.set_processed(raw, processed),
        }
    }

    assets.requeue(still_pending);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assets::upload::AssetSource, ecs::schedule::Schedule};

    struct TestBackend(i32);

    struct DepMarker(i32);

    struct TestAsset(i32);

    impl AssetSource for TestAsset {
        type Processed = i32;
    }

    impl Asset<TestBackend> for TestAsset {
        type Deps<'a> = Read<'a, DepMarker>;

        fn upload<'a>(&self, backend: &TestBackend, deps: &Self::Deps<'a>) -> Option<i32> {
            Some(self.0 * backend.0 * deps.0)
        }
    }

    #[test]
    fn upload_waits_for_backend_and_deps_then_processes() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Assets::<TestAsset>::new());

        let handle = resources.get_mut::<Assets<TestAsset>>().insert("test", TestAsset(3));

        let mut schedule = Schedule::default();
        schedule.add_system(sync_assets::<TestBackend, TestAsset>);

        schedule.run(&mut world, &mut resources);
        assert!(!resources.get::<Assets<TestAsset>>().is_ready(handle));

        resources.insert(TestBackend(2));
        schedule.run(&mut world, &mut resources);
        assert!(!resources.get::<Assets<TestAsset>>().is_ready(handle), "should still wait on the missing dependency");

        resources.insert(DepMarker(5));
        schedule.run(&mut world, &mut resources);
        assert!(resources.get::<Assets<TestAsset>>().is_ready(handle));
        assert_eq!(*resources.get::<Assets<TestAsset>>().get(handle).unwrap(), 3 * 2 * 5);
    }
}
