use crate::{
    app::SystemStage,
    assets::{
        deps::Dependencies,
        storage::{Assets, ProcessedAssets},
        upload::DeviceUpload,
    },
    ecs::{plugin::Plugin, resources::Resources, system::{Res, ResMut}},
};

pub struct DeviceAssetPlugin<D, T: DeviceUpload<D>> {
    _marker: std::marker::PhantomData<(D, T)>,
}

impl<D, T: DeviceUpload<D>> DeviceAssetPlugin<D, T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<D, T> Plugin for DeviceAssetPlugin<D, T>
where
    D: 'static + Send + Sync,
    T: DeviceUpload<D>,
{
    fn build(&self, app: &mut crate::app::App) {
        app.try_insert_resource(Assets::<T::Source>::new());
        app.try_insert_resource(ProcessedAssets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_device_assets::<D, T>);
        app.required.provides::<ProcessedAssets<T>>();
    }
}

fn sync_device_assets<D, T>(
    mut cpu: ResMut<Assets<T::Source>>,
    mut processed: ResMut<ProcessedAssets<T>>,
    device: Option<Res<D>>,
    world: &hecs::World,
    resources: &Resources,
) where
    D: 'static + Send + Sync,
    T: DeviceUpload<D>,
{
    let Some(device) = device else {
        log_waiting::<D, T>(&cpu, "device");
        return;
    };
    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<D, T>(&cpu, "dependencies");
        return;
    };
    for handle in cpu.take_dirty() {
        if let Some(source) = cpu.get(handle) {
            processed.insert(handle, T::upload(source, &device, &deps));
        }
    }
}

fn log_waiting<D, T>(cpu: &Assets<T::Source>, what: &str)
where
    D: 'static + Send + Sync,
    T: DeviceUpload<D>,
{
    if !cpu.dirty_is_empty() {
        tracing::trace!(
            "{}: waiting on {what}, {} pending",
            std::any::type_name::<T>(),
            cpu.dirty_len()
        );
    }
}
