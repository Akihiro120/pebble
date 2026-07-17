use slotmap::{SecondaryMap, SlotMap, new_key_type};
use std::collections::HashMap;

new_key_type! {
    /// Untyped slot-map key for an asset entry.
    ///
    /// Prefer the typed [`Handle<T>`](crate::assets::handle::Handle) over this
    /// in most code. `RawAssetHandle` is used internally by the storage and
    /// sync systems.
    pub struct RawAssetHandle;
}

/// Storage for raw CPU-side assets of type `T`.
///
/// Assets are inserted by name and looked up by either name or
/// [`RawAssetHandle`]. When an asset is inserted or updated its handle is
/// pushed onto the *dirty queue*, which the sync system drains each tick to
/// upload changed assets to the GPU.
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

    /// Insert `asset` under `name`, returning its handle.
    ///
    /// If an asset with the same name already exists it is replaced and the
    /// old entry is removed from the slot-map and dirty queue.
    pub fn insert(&mut self, name: &str, asset: T) -> RawAssetHandle {
        let handle = self.storage.insert(asset);
        self.queue.push(handle);

        if let Some(old) = self.handles.insert(name.to_string(), handle) {
            self.storage.remove(old);
            self.queue.retain(|h| *h != old);
        }
        handle
    }

    /// Look up an asset by its raw handle.
    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    /// Mutably look up an asset by its raw handle.
    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    /// Look up an asset by its name.
    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        self.handles
            .get(name)
            .and_then(|&handle| self.storage.get(handle))
    }

    /// Mutably look up an asset by its name.
    pub fn get_mut_by_name(&mut self, name: &str) -> Option<&mut T> {
        let handle = self.handles.get(name).copied()?;
        self.storage.get_mut(handle)
    }

    /// Drain and return all handles currently in the dirty queue.
    ///
    /// Called by the asset sync system each tick.
    pub fn take_dirty(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.queue)
    }

    /// Remove an asset by handle, returning the value if it existed.
    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        let value = self.storage.remove(handle)?;

        self.handles.retain(|_, h| *h != handle);
        self.queue.retain(|h| *h != handle);

        Some(value)
    }

    /// Remove an asset by name, returning the value if it existed.
    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let handle = self.handles.remove(name)?;
        self.storage.remove(handle)
    }

    /// Returns `true` if the dirty queue is empty.
    pub fn dirty_is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the number of handles currently in the dirty queue.
    pub fn dirty_len(&self) -> usize {
        self.queue.len()
    }

    /// Push `handles` back onto the dirty queue so they are retried next tick.
    pub fn requeue(&mut self, handles: Vec<RawAssetHandle>) {
        self.queue.extend(handles);
    }
}

/// Storage for backend-processed (GPU) assets indexed by the same
/// [`RawAssetHandle`] as their source in [`Assets`].
///
/// Populated by the asset sync system after a successful [`Asset::upload`](crate::assets::upload::Asset::upload).
pub struct ProcessedAssets<T: 'static + Send + Sync> {
    storage: SecondaryMap<RawAssetHandle, T>,
}

impl<T: 'static + Send + Sync> ProcessedAssets<T> {
    pub fn new() -> Self {
        Self {
            storage: SecondaryMap::new(),
        }
    }

    /// Store a processed asset, returning the previous value if one existed.
    pub fn insert(&mut self, handle: RawAssetHandle, asset: T) -> Option<T> {
        self.storage.insert(handle, asset)
    }

    /// Look up a processed asset by handle.
    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        self.storage.get(handle)
    }

    /// Mutably look up a processed asset by handle.
    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
        self.storage.get_mut(handle)
    }

    /// Remove a processed asset by handle, returning the value if it existed.
    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        self.storage.remove(handle)
    }

    /// Returns `true` if a processed asset exists for `handle`.
    pub fn contains(&self, handle: RawAssetHandle) -> bool {
        self.storage.contains_key(handle)
    }
}
