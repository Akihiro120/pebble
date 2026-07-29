use slotmap::{Key as _, SecondaryMap, SlotMap, new_key_type};
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
    removed: Vec<RawAssetHandle>,
}

impl<T: 'static + Send + Sync> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Insert `asset` under `name`, returning its handle.
    ///
    /// If an asset with the same name already exists, its data is replaced
    /// **in-place**: the same slot-map entry (and therefore the same handle)
    /// is reused, so any code that already holds a [`Handle`](crate::assets::handle::Handle)
    /// or [`RawAssetHandle`] for this asset continues to work correctly.
    /// The handle is re-queued so the sync system re-uploads the new data.
    pub fn insert(&mut self, name: &str, asset: T) -> RawAssetHandle {
        if let Some(&existing) = self.handles.get(name) {
            // Replace data in-place so existing handles stay valid.
            if let Some(slot) = self.storage.get_mut(existing) {
                *slot = asset;
                // Only queue once — don't duplicate if already dirty.
                if !self.queue.contains(&existing) {
                    self.queue.push(existing);
                }
                tracing::debug!(
                    "Assets<{}>: replaced data for {:?} ({name}) in-place",
                    std::any::type_name::<T>(),
                    existing
                );
                return existing;
            }
        }

        let handle = self.storage.insert(asset);
        self.handles.insert(name.to_string(), handle);
        self.queue.push(handle);
        handle
    }

    /// Look up an asset by its raw handle.
    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        if handle.is_null() {
            tracing::warn!(
                "Assets<{}>: get() called with a null/default handle — \
                 did you forget to insert the asset and store the returned handle?",
                std::any::type_name::<T>()
            );
            return None;
        }
        let result = self.storage.get(handle);
        if result.is_none() {
            tracing::warn!(
                "Assets<{}>: get() called with a stale handle {:?} — \
                 the asset was likely removed or replaced since this handle was obtained",
                std::any::type_name::<T>(),
                handle
            );
        }
        result
    }

    /// Mutably look up an asset by its raw handle.
    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
        if handle.is_null() {
            tracing::warn!(
                "Assets<{}>: get_mut() called with a null/default handle — \
                 did you forget to insert the asset and store the returned handle?",
                std::any::type_name::<T>()
            );
            return None;
        }
        let result = self.storage.get_mut(handle);
        if result.is_none() {
            tracing::warn!(
                "Assets<{}>: get_mut() called with a stale handle {:?} — \
                 the asset was likely removed or replaced since this handle was obtained",
                std::any::type_name::<T>(),
                handle
            );
        }
        result
    }

    /// Look up an asset by its name.
    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        let result = self.handles
            .get(name)
            .and_then(|&handle| self.storage.get(handle));
        if result.is_none() {
            tracing::debug!(
                "Assets<{}>: get_by_name({:?}) found no asset — \
                 the name may not have been inserted yet",
                std::any::type_name::<T>(),
                name
            );
        }
        result
    }

    /// Mutably look up an asset by its name.
    pub fn get_mut_by_name(&mut self, name: &str) -> Option<&mut T> {
        let handle = self.handles.get(name).copied();
        if handle.is_none() {
            tracing::debug!(
                "Assets<{}>: get_mut_by_name({:?}) found no asset — \
                 the name may not have been inserted yet",
                std::any::type_name::<T>(),
                name
            );
            return None;
        }
        self.storage.get_mut(handle.unwrap())
    }

    /// Look up an asset handle by its name.
    pub fn get_handle_by_name(&self, name: &str) -> Option<RawAssetHandle> {
        self.handles.get(name).copied()
    }

    /// Drain and return all handles currently in the dirty queue.
    ///
    /// Called by the asset sync system each tick.
    pub fn take_dirty(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.queue)
    }

    /// Remove an asset by handle, returning the value if it existed.
    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        if handle.is_null() {
            tracing::warn!(
                "Assets<{}>: remove() called with a null/default handle — no-op",
                std::any::type_name::<T>()
            );
            return None;
        }
        let value = self.storage.remove(handle)?;

        self.handles.retain(|_, h| *h != handle);
        self.queue.retain(|h| *h != handle);
        self.removed.push(handle);

        Some(value)
    }

    /// Remove an asset by name, returning the value if it existed.
    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let handle = self.handles.remove(name)?;
        self.queue.retain(|h| *h != handle);
        self.removed.push(handle);
        self.storage.remove(handle)
    }

    /// Drain and return all handles removed since the last call.
    ///
    /// Called by the asset sync system each tick to evict stale processed assets.
    pub fn take_removed(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.removed)
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

    /// Replace the data for an existing asset by handle, re-queuing it for
    /// upload. Returns `false` if the handle is null or not present.
    ///
    /// This is the handle-based counterpart to [`insert`](Self::insert): use it
    /// when you already have the handle and don't need to go through a name
    /// lookup. The handle stays valid — only the underlying data changes.
    pub fn replace(&mut self, handle: RawAssetHandle, asset: T) -> bool {
        if handle.is_null() {
            tracing::warn!(
                "Assets<{}>: replace() called with a null/default handle — no-op",
                std::any::type_name::<T>()
            );
            return false;
        }
        let Some(slot) = self.storage.get_mut(handle) else {
            tracing::warn!(
                "Assets<{}>: replace() called with a stale handle {:?} — no-op",
                std::any::type_name::<T>(),
                handle
            );
            return false;
        };
        *slot = asset;
        if !self.queue.contains(&handle) {
            self.queue.push(handle);
        }
        tracing::debug!(
            "Assets<{}>: replaced data for {:?}{} via handle",
            std::any::type_name::<T>(),
            handle,
            self.name_for_handle(handle)
                .map(|n| format!(" ({n})"))
                .unwrap_or_default()
        );
        true
    }

    /// Mark a single asset as dirty so the sync system re-uploads it next tick,
    /// even though its source data has not changed.
    ///
    /// Use this when a resource the asset *depends on* has been recreated (e.g.
    /// a texture that a material instance's bind group references was resized),
    /// requiring the processed asset to be rebuilt with the new version.
    ///
    /// Does nothing if `handle` is null or not present in this store.
    pub fn mark_dirty(&mut self, handle: RawAssetHandle) {
        if handle.is_null() {
            tracing::warn!(
                "Assets<{}>: mark_dirty() called with a null/default handle — no-op",
                std::any::type_name::<T>()
            );
            return;
        }
        if self.storage.contains_key(handle) && !self.queue.contains(&handle) {
            self.queue.push(handle);
        }
    }

    /// Iterate over all assets by handle.
    pub fn iter(&self) -> impl Iterator<Item = (RawAssetHandle, &T)> {
        self.storage.iter()
    }

    /// Mutably iterate over all assets by handle.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (RawAssetHandle, &mut T)> {
        self.storage.iter_mut()
    }

    /// Iterate over `(name, handle)` pairs for every named asset.
    pub fn names(&self) -> impl Iterator<Item = (&str, RawAssetHandle)> {
        self.handles
            .iter()
            .map(|(name, &handle)| (name.as_str(), handle))
    }

    pub fn name_for_handle(&self, handle: RawAssetHandle) -> Option<&str> {
        self.handles
            .iter()
            .find(|(_, h)| **h == handle)
            .map(|(name, _)| name.as_str())
    }
}

impl<'a, T: 'static + Send + Sync> IntoIterator for &'a Assets<T> {
    type Item = (RawAssetHandle, &'a T);
    type IntoIter = slotmap::basic::Iter<'a, RawAssetHandle, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter()
    }
}

impl<'a, T: 'static + Send + Sync> IntoIterator for &'a mut Assets<T> {
    type Item = (RawAssetHandle, &'a mut T);
    type IntoIter = slotmap::basic::IterMut<'a, RawAssetHandle, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter_mut()
    }
}

/// Storage for backend-processed (GPU) assets indexed by the same
/// [`RawAssetHandle`] as their source in [`Assets`].
///
/// Populated by the asset sync system after a successful [`Asset::upload`](crate::assets::upload::Asset::upload).
pub struct ProcessedAssets<T: 'static + Send + Sync> {
    storage: SecondaryMap<RawAssetHandle, T>,
    pub(crate) names: HashMap<String, RawAssetHandle>,
}

impl<T: 'static + Send + Sync> ProcessedAssets<T> {
    pub fn new() -> Self {
        Self {
            storage: SecondaryMap::new(),
            names: HashMap::new(),
        }
    }

    /// Store a processed asset, returning the previous value if one existed.
    pub fn insert(&mut self, handle: RawAssetHandle, asset: T) -> Option<T> {
        self.storage.insert(handle, asset)
    }

    /// Look up a processed asset by handle.
    pub fn get(&self, handle: RawAssetHandle) -> Option<&T> {
        if handle.is_null() {
            tracing::warn!(
                "ProcessedAssets<{}>: get() called with a null/default handle — \
                 did you forget to insert the source asset and store the returned handle?",
                std::any::type_name::<T>()
            );
            return None;
        }
        let result = self.storage.get(handle);
        if result.is_none() {
            tracing::debug!(
                "ProcessedAssets<{}>: get() for handle {:?} returned nothing — \
                 the asset may still be pending upload or was removed",
                std::any::type_name::<T>(),
                handle
            );
        }
        result
    }

    /// Mutably look up a processed asset by handle.
    pub fn get_mut(&mut self, handle: RawAssetHandle) -> Option<&mut T> {
        if handle.is_null() {
            tracing::warn!(
                "ProcessedAssets<{}>: get_mut() called with a null/default handle — \
                 did you forget to insert the source asset and store the returned handle?",
                std::any::type_name::<T>()
            );
            return None;
        }
        let result = self.storage.get_mut(handle);
        if result.is_none() {
            tracing::debug!(
                "ProcessedAssets<{}>: get_mut() for handle {:?} returned nothing — \
                 the asset may still be pending upload or was removed",
                std::any::type_name::<T>(),
                handle
            );
        }
        result
    }

    /// Remove a processed asset by handle, returning the value if it existed.
    pub fn remove(&mut self, handle: RawAssetHandle) -> Option<T> {
        self.names.retain(|_, h| *h != handle);
        self.storage.remove(handle)
    }

    /// Returns `true` if a processed asset exists for `handle`.
    pub fn contains(&self, handle: RawAssetHandle) -> bool {
        self.storage.contains_key(handle)
    }

    /// Iterate over all processed assets by handle.
    pub fn iter(&self) -> impl Iterator<Item = (RawAssetHandle, &T)> {
        self.storage.iter()
    }

    /// Mutably iterate over all processed assets by handle.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (RawAssetHandle, &mut T)> {
        self.storage.iter_mut()
    }

    pub fn get_by_name(&self, name: &str) -> Option<&T> {
        let handle = self.names.get(name)?;
        self.storage.get(*handle)
    }
}

impl<'a, T: 'static + Send + Sync> IntoIterator for &'a ProcessedAssets<T> {
    type Item = (RawAssetHandle, &'a T);
    type IntoIter = slotmap::secondary::Iter<'a, RawAssetHandle, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter()
    }
}

impl<'a, T: 'static + Send + Sync> IntoIterator for &'a mut ProcessedAssets<T> {
    type Item = (RawAssetHandle, &'a mut T);
    type IntoIter = slotmap::secondary::IterMut<'a, RawAssetHandle, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.storage.iter_mut()
    }
}
