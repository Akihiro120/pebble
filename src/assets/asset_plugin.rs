use crate::{
    app::SystemStage,
    assets::{
        dependent_asset_plugin::Dependencies,
        storage::{Assets, ProcessedAssets},
        upload::DeviceUpload,
    },
    plugin::Plugin,
    rendering::backend::Backend,
    resources::Resources,
    system::{Res, ResMut},
};

pub struct DeviceAssetPlugin<B: Backend, T: DeviceUpload<B>> {
    _marker: std::marker::PhantomData<(B, T)>,
}

impl<B: Backend, T: DeviceUpload<B>> DeviceAssetPlugin<B, T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B, T> Plugin for DeviceAssetPlugin<B, T>
where
    B: Backend,
    T: DeviceUpload<B>,
{
    fn build(&self, app: &mut crate::prelude::App) {
        app.try_insert_resource(Assets::<T::Source>::new());
        app.try_insert_resource(ProcessedAssets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_device_assets::<B, T>);
    }
}

fn sync_device_assets<B, T>(
    mut cpu: ResMut<Assets<T::Source>>,
    mut gpu: ResMut<ProcessedAssets<T>>,
    backend: Option<Res<B>>,
    world: &hecs::World,
    resources: &Resources,
) where
    B: Backend,
    T: DeviceUpload<B>,
{
    let Some(device) = backend else {
        log_waiting::<B, T>(&cpu, "backend");
        return;
    };

    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<B, T>(&cpu, "dependencies");
        return;
    };

    for handle in cpu.take_dirty() {
        if let Some(source) = cpu.get(handle) {
            gpu.insert(handle, T::upload(source, &device, &deps));
        }
    }
}

fn log_waiting<B, T>(cpu: &Assets<T::Source>, what: &str)
where
    B: Backend,
    T: DeviceUpload<B>,
{
    if !cpu.dirty_is_empty() {
        tracing::trace!(
            "{}: waiting on {what}, {} pending",
            std::any::type_name::<T>(),
            cpu.dirty_len()
        );
    }
}
