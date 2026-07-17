use slotmap::{SecondaryMap, SlotMap, new_key_type};
use std::collections::HashMap;

new_key_type! {
    pub struct RawAssetHandle;
}

pub struct Assets<T: 'static + Send + Sync> {
    storage: SlotMap<RawAssetHandle, T>,
    handles: HashMap<String, RawAssetHandle>,
    queue: Vec<RawAssetHandle>,
}

impl<T: 'static + Send + Sync> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
        }
    }

    pub fn insert(&mut self, name: &str, asset: T) -> RawAssetHandle {
        let handle = self.storage.insert(asset);
        self.queue.push(handle);

        if let Some(old) = self.handles.insert(name.to_string(), handle) {
            self.storage.remove(old);
            self.queue.retain(|h| *h != old);
        }
        handle
    }

    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
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

    pub fn take_dirty(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.queue)
    }

    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        let value = self.storage.remove(handle)?;

        self.handles.retain(|_, h| *h != handle);
        self.queue.retain(|h| *h != handle);

        Some(value)
    }

    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let handle = self.handles.remove(name)?;
        self.storage.remove(handle)
    }

    pub fn dirty_is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn dirty_len(&self) -> usize {
        self.queue.len()
    }

    pub fn requeue(&mut self, handles: Vec<RawAssetHandle>) {
        self.queue.extend(handles);
    }
}

pub struct ProcessedAssets<T: 'static + Send + Sync> {
    storage: SecondaryMap<RawAssetHandle, T>,
}

impl<T: 'static + Send + Sync> ProcessedAssets<T> {
    pub fn new() -> Self {
        Self {
            storage: SecondaryMap::new(),
        }
    }

    pub fn insert(&mut self, handle: RawAssetHandle, asset: T) -> Option<T> {
        self.storage.insert(handle, asset)
    }

    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        self.storage.remove(handle)
    }

    pub fn contains(&self, handle: RawAssetHandle) -> bool {
        self.storage.contains_key(handle)
    }
}
