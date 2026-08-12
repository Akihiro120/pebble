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

/// Holds every loaded asset of type `T` — both its CPU-side source and (once
/// uploaded) its GPU-side processed form, together. Registering
/// `AssetPlugin::<Backend, T>::new()` drains newly-inserted/dirty entries
/// automatically each tick; `insert`/`get`/`get_source` are the everyday API.
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

    /// Inserts `source` under `name`, queuing it for upload. Inserting
    /// under a name that already exists replaces that entry (and marks it
    /// dirty again) instead of creating a second one.
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

    /// The GPU-side processed value, if it's finished uploading.
    pub fn get(&self, handle: Handle<T>) -> Option<&T::Processed> {
        self.storage.get(handle.id)?.processed.as_ref()
    }

    /// The CPU-side source value — e.g. a `Mesh`'s vertices/indices, for
    /// building a collision mesh from the same data used to render it.
    pub fn get_source(&self, handle: Handle<T>) -> Option<&T> {
        self.storage.get(handle.id).map(|entry| &entry.source)
    }

    pub fn get_source_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.storage.get_mut(handle.id).map(|entry| &mut entry.source)
    }

    /// Whether this handle's GPU-side upload has finished.
    pub fn is_ready(&self, handle: Handle<T>) -> bool {
        self.storage.get(handle.id).is_some_and(|entry| entry.processed.is_some())
    }

    /// Whether `handle` still refers to a live entry.
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.storage.contains_key(handle.id)
    }

    /// Re-queues an entry for upload — e.g. after mutating it via
    /// [`get_source_mut`](Self::get_source_mut).
    pub fn mark_dirty(&mut self, handle: Handle<T>) {
        if self.storage.contains_key(handle.id) && !self.queue.contains(&handle.id) {
            self.queue.push(handle.id);
        }
    }

    /// Same as [`get`](Self::get), looked up by the name it was
    /// [`insert`](Self::insert)ed under.
    pub fn get_by_name(&self, name: &str) -> Option<&T::Processed> {
        let handle = self.handles.get(name)?;
        self.storage.get(*handle)?.processed.as_ref()
    }

    /// Same as [`get_source`](Self::get_source), looked up by name.
    pub fn get_source_by_name(&self, name: &str) -> Option<&T> {
        let handle = self.handles.get(name)?;
        Some(&self.storage.get(*handle)?.source)
    }

    /// The [`Handle<T>`] an asset was inserted under, by name.
    pub fn get_handle_by_name(&self, name: &str) -> Option<Handle<T>> {
        self.handles.get(name).copied().map(Handle::new)
    }

    /// Every currently-loaded asset of this type, source data included —
    /// e.g. to build a static collision world from every loaded `Mesh`, or
    /// a debug asset browser. Ready-or-not — pair with
    /// [`is_ready`](Self::is_ready) if you only want finished uploads.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.storage.iter().map(|(id, entry)| (Handle::new(id), &entry.source))
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
