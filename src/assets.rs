use std::collections::HashMap;

use slotmap::{SecondaryMap, SlotMap, new_key_type};

use crate::{Plugin, Res, ResMut};

pub trait Asset: 'static {
    type Intermediate: 'static + Send + Sync;

    fn source(&self) -> Self::Intermediate;
}

pub trait AssetLoader: 'static {
    type Asset: 'static + Send + Sync;
    type GPUCtx: 'static + Send + Sync;
    type GPUOutput: 'static + Send + Sync;

    fn sync(&self, asset: &mut Self::Asset, gpu: &Self::GPUCtx) -> Self::GPUOutput;
}

new_key_type! {
    pub struct AssetHandle;
}

pub struct Assets<T> {
    storage: SlotMap<AssetHandle, T>,
    handles: HashMap<String, AssetHandle>,
    queue: Vec<AssetHandle>,
}

impl<T> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
        }
    }

    pub fn insert<A: Asset<Intermediate = T>>(&mut self, name: &str, asset: A) -> AssetHandle {
        let data = asset.source();

        let handle = self.storage.insert(data);

        self.queue.push(handle);
        self.handles.insert(name.to_string(), handle);

        handle
    }
}

pub struct GPUAssets<T> {
    storage: SecondaryMap<AssetHandle, T>,
}

impl<T> GPUAssets<T> {
    pub fn new() -> Self {
        Self {
            storage: SecondaryMap::new(),
        }
    }
}

pub struct AssetPlugin<L> {
    loader: L,
}

impl<L> AssetPlugin<L> {
    pub fn new(loader: L) -> Self {
        Self { loader }
    }
}

impl<L> Plugin for AssetPlugin<L>
where
    L: AssetLoader + Clone + Send + Sync,
{
    fn build(&self, app: &mut crate::App) {
        // build cpu asset storage
        // build gpu asset storage
        // add loader

        app.try_insert_resource(self.loader.clone());
        app.try_insert_resource(Assets::<L::Asset>::new());
        app.try_insert_resource(GPUAssets::<L::GPUOutput>::new());

        app.add_system(crate::SystemStage::PreUpdate, asset_sync_system::<L>);
    }
}

fn asset_sync_system<L>(
    mut cpu: ResMut<Assets<L::Asset>>,
    mut gpu: ResMut<GPUAssets<L::GPUOutput>>,
    loader: Res<L>,
    gpu_ctx: Res<L::GPUCtx>,
) where
    L: AssetLoader + Send + Sync,
{
    let queue = std::mem::take(&mut cpu.queue);

    for handle in queue {
        if let Some(cpu_asset) = cpu.storage.get_mut(handle) {
            let gpu_asset = loader.sync(cpu_asset, &gpu_ctx);
            gpu.storage.insert(handle, gpu_asset);
        }
    }
}
