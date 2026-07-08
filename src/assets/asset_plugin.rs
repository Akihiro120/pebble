use crate::{
    app::SystemStage,
    assets::{
        storage::{Assets, GPUAssets},
        upload::DeviceUpload,
    },
    plugin::Plugin,
    rendering::backend::Backend,
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
        app.try_insert_resource(GPUAssets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_device_assets::<B, T>);
    }
}

fn sync_device_assets<B, T>(
    mut cpu: ResMut<Assets<T::Source>>,
    mut gpu: ResMut<GPUAssets<T>>,
    backend: Option<Res<B>>,
) where
    B: Backend,
    T: DeviceUpload<B>,
{
    let Some(device) = backend else { return };
    for handle in cpu.take_dirty() {
        if let Some(source) = cpu.get(handle) {
            gpu.insert(handle, T::upload(source, &device));
        } else {
            tracing::warn!("Dirty asset handle removed before GPU sync");
        }
    }
}
