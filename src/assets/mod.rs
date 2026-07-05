use crate::{Assets, Backend, DeviceUpload, GPUAssets, Plugin, Res, ResMut, SystemStage};

pub mod handle;
pub mod storage;
pub mod upload;

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
        }
    }
}

macro_rules! impl_dependent_asset {
    ($plugin:ident, $trait_name:ident, $sync_fn:ident, $($dep:ident),+) => {
        pub trait $trait_name<D, $($dep),+>: 'static + Send + Sync + Sized
        where $($dep: 'static + Send + Sync),+
        {
            type Source: 'static + Send + Sync;
            fn upload(source: &Self::Source, device: &D, $($dep: &$dep),+) -> Self;
        }

        pub struct $plugin<D, T, $($dep),+>(std::marker::PhantomData<(D, T, $($dep),+)>);
        impl<D, T, $($dep),+> $plugin<D, T, $($dep),+> {
            pub fn new() -> Self { Self(std::marker::PhantomData) }
        }
        impl<D, T, $($dep),+> Plugin for $plugin<D, T, $($dep),+>
        where
            D: 'static + Send + Sync,
            $($dep: 'static + Send + Sync,)+
            T: $trait_name<D, $($dep),+>,
        {
            fn build(&self, app: &mut crate::app::App) {
                app.try_insert_resource(Assets::<T::Source>::new());
                app.try_insert_resource(GPUAssets::<T>::new());
                app.add_system(SystemStage::AssetSyncDeps, $sync_fn::<D, T, $($dep),+>);
            }
        }
        fn $sync_fn<D, T, $($dep),+>(
            mut cpu: ResMut<Assets<T::Source>>,
            mut gpu: ResMut<GPUAssets<T>>,
            device: Option<Res<D>>,
            $($dep: Option<Res<$dep>>),+
        ) where
            D: 'static + Send + Sync,
            $($dep: 'static + Send + Sync,)+
            T: $trait_name<D, $($dep),+>,
        {
            let Some(device) = device else { return };
            $(let Some($dep) = $dep else { return };)+

            for handle in cpu.take_dirty() {
                if let Some(source) = cpu.get(handle) {
                    gpu.insert(handle, T::upload(source, &device, $(&$dep),+));
                }
            }
        }
    };
}

impl_dependent_asset!(
    DependentAssetPlugin1,
    DependentUpload1,
    sync_dependent1,
    Dep1
);
impl_dependent_asset!(
    DependentAssetPlugin2,
    DependentUpload2,
    sync_dependent2,
    Dep1,
    Dep2
);
impl_dependent_asset!(
    DependentAssetPlugin3,
    DependentUpload3,
    sync_dependent3,
    Dep1,
    Dep2,
    Dep3
);
impl_dependent_asset!(
    DependentAssetPlugin4,
    DependentUpload4,
    sync_dependent4,
    Dep1,
    Dep2,
    Dep3,
    Dep4
);
