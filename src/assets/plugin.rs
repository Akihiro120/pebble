use crate::{
    app::SystemStage,
    assets::{
        deps::Dependencies,
        storage::{Assets, ProcessedAssets},
        upload::Asset,
    },
    ecs::{plugin::Plugin, resources::Resources, system::{Res, ResMut}},
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
    T: Asset<B>,
{
    fn build(&self, app: &mut crate::app::App) {
        app.try_insert_resource(Assets::<T::Source>::new());
        app.try_insert_resource(ProcessedAssets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_assets::<B, T>);
        app.required.provides::<ProcessedAssets<T>>();
    }
}

fn sync_assets<B, T>(
    mut cpu: ResMut<Assets<T::Source>>,
    mut processed: ResMut<ProcessedAssets<T>>,
    backend: Option<Res<B>>,
    world: &hecs::World,
    resources: &Resources,
) where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    let Some(backend) = backend else {
        log_waiting::<B, T>(&cpu, "backend");
        return;
    };
    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<B, T>(&cpu, "dependencies");
        return;
    };
    for handle in cpu.take_dirty() {
        if let Some(source) = cpu.get(handle) {
            processed.insert(handle, T::upload(source, &backend, &deps));
        }
    }
}

fn log_waiting<B, T>(cpu: &Assets<T::Source>, what: &str)
where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    if !cpu.dirty_is_empty() {
        tracing::trace!(
            "{}: waiting on {what}, {} pending",
            std::any::type_name::<T>(),
            cpu.dirty_len()
        );
    }
}
