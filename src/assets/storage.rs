use std::collections::HashMap;

use slotmap::{SlotMap, new_key_type};

use crate::assets::{handle::Handle, upload::AssetSource};

new_key_type! {
    pub struct RawAssetHandle;
}

struct AssetEntry<T: AssetSource> {
    source: T,
    processed: Option<T::Processed>,
}

pub struct Assets<T: AssetSource> {
    storage: SlotMap<RawAssetHandle, AssetEntry<T>>,
    handles: HashMap<String, RawAssetHandle>,
    queue: Vec<RawAssetHandle>,
}

impl<T: AssetSource> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
        }
    }

    pub fn insert(&mut self, name: &str, source: T) -> Handle<T> {
        if let Some(&existing) = self.handles.get(name) {
            if let Some(entry) = self.storage.get_mut(existing) {
                entry.source = source;
                entry.processed = None;
                if !self.queue.contains(&existing) {
                    self.queue.push(existing);
                }
                return Handle::new(existing);
            }
        }
        let handle = self.storage.insert(AssetEntry { source, processed: None });
        self.handles.insert(name.to_string(), handle);
        self.queue.push(handle);
        Handle::new(handle)
    }

    pub fn get(&self, handle: Handle<T>) -> Option<&T::Processed> {
        self.storage.get(handle.id)?.processed.as_ref()
    }

    pub fn get_source(&self, handle: Handle<T>) -> Option<&T> {
        self.storage.get(handle.id).map(|entry| &entry.source)
    }

    pub fn get_source_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storage.get_mut(handle.id).map(|entry| &mut entry.source)
    }

    pub fn is_ready(&self, handle: Handle<T>) -> bool {
        self.storage.get(handle.id).is_some_and(|entry| entry.processed.is_some())
    }

    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.storage.contains_key(handle.id)
    }

    pub fn mark_dirty(&mut self, handle: Handle<T>) {
        if self.storage.contains_key(handle.id) && !self.queue.contains(&handle.id) {
            self.queue.push(handle.id);
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<&T::Processed> {
        let handle = self.handles.get(name)?;
        self.storage.get(*handle)?.processed.as_ref()
    }

    pub fn get_source_by_name(&self, name: &str) -> Option<&T> {
        let handle = self.handles.get(name)?;
        Some(&self.storage.get(*handle)?.source)
    }

    pub fn get_handle_by_name(&self, name: &str) -> Option<Handle<T>> {
        self.handles.get(name).copied().map(Handle::new)
    }

    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        self.queue.retain(|&id| id != handle.id);
        self.handles.retain(|_, &mut id| id != handle.id);
        self.storage.remove(handle.id).map(|entry| entry.source)
    }

    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let id = self.handles.remove(name)?;
        self.queue.retain(|&h| h != id);
        self.storage.remove(id).map(|entry| entry.source)
    }

    pub(crate) fn set_processed(&mut self, handle: RawAssetHandle, processed: T::Processed) {
        if let Some(entry) = self.storage.get_mut(handle) {
            entry.processed = Some(processed);
        }
    }

    pub(crate) fn take_dirty(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.queue)
    }

    pub(crate) fn requeue(&mut self, handles: Vec<RawAssetHandle>) {
        self.queue.extend(handles);
    }
}

impl<T: AssetSource> Default for Assets<T> {
    fn default() -> Self {
        Self::new()
    }
}
