use slotmap::{SecondaryMap, SlotMap, new_key_type};
use std::collections::HashMap;

new_key_type! {
    pub struct AssetHandle;
}

pub struct Assets<T: 'static + Send + Sync> {
    storage: SlotMap<AssetHandle, T>,
    handles: HashMap<String, AssetHandle>,
    queue: Vec<AssetHandle>,
}

impl<T: 'static + Send + Sync> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
        }
    }

    pub fn insert(&mut self, name: &str, asset: T) -> AssetHandle {
        let handle = self.storage.insert(asset);
        self.queue.push(handle);
        self.handles.insert(name.to_string(), handle);
        handle
    }

    pub fn get(&self, handle: AssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: AssetHandle) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        self.handles
            .get(name)
            .and_then(|&handle| self.storage.get(handle))
    }

    pub fn get_mut_by_name(&mut self, name: &str) -> Option<&mut T> {
        let handle = self.handles.get(name).copied()?;
        self.storage.get_mut(handle)
    }

    pub fn take_dirty(&mut self) -> Vec<AssetHandle> {
        std::mem::take(&mut self.queue)
    }

    pub fn remove(&mut self, handle: AssetHandle) -> Option<T> {
        let value = self.storage.remove(handle)?;

        self.handles.retain(|_, h| *h != handle);
        self.queue.retain(|h| *h != handle);

        Some(value)
    }

    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let handle = self.handles.remove(name)?;
        self.storage.remove(handle)
    }
}

pub struct GPUAssets<T: 'static + Send + Sync> {
    storage: SecondaryMap<AssetHandle, T>,
}

impl<T: 'static + Send + Sync> GPUAssets<T> {
    pub fn new() -> Self {
        Self {
            storage: SecondaryMap::new(),
        }
    }

    pub fn insert(&mut self, handle: AssetHandle, asset: T) -> Option<T> {
        self.storage.insert(handle, asset)
    }

    pub fn get(&self, handle: AssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: AssetHandle) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    pub fn remove(&mut self, handle: AssetHandle) -> Option<T> {
        self.storage.remove(handle)
    }

    pub fn contains(&self, handle: AssetHandle) -> bool {
        self.storage.contains_key(handle)
    }
}
