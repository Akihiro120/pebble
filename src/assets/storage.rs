use slotmap::{Key as _, SlotMap, new_key_type};
use std::collections::HashMap;

use crate::assets::{handle::Handle, upload::AssetSource};

new_key_type! {
    /// Untyped slot-map key for an asset entry.
    ///
    /// Prefer the typed [`Handle<T>`](crate::assets::handle::Handle) in
    /// most code. `RawAssetHandle` is used internally by the storage and
    /// sync systems.
    pub struct RawAssetHandle;
}

fn warn_if_null<T>(handle: RawAssetHandle, method: &str, on_null: &str) -> bool {
    if handle.is_null() {
        tracing::warn!(
            "Assets<{}>: {method}() called with a null/default handle — {on_null}",
            std::any::type_name::<T>()
        );
        true
    } else {
        false
    }
}

struct AssetEntry<T: AssetSource> {
    source: T,
    processed: Option<T::Processed>,
}

/// Unified storage for source and processed assets of type `T`.
///
/// Each entry holds both the raw source data (`T`) and the uploaded result
/// (`T::Processed`), keyed by the same [`Handle<T>`].
/// [`AssetPlugin`](crate::assets::plugin::AssetPlugin) fills in `processed`
/// after a successful [`Asset::upload`](crate::assets::upload::Asset::upload).
///
/// Use [`get`](Self::get) to retrieve the processed result (e.g. for
/// rendering), and [`get_source`](Self::get_source) to access the raw data.
pub struct Assets<T: AssetSource> {
    storage: SlotMap<RawAssetHandle, AssetEntry<T>>,
    handles: HashMap<String, RawAssetHandle>,
    queue: Vec<RawAssetHandle>,
    removed: Vec<RawAssetHandle>,
}

impl<T: AssetSource> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SlotMap::with_key(),
            handles: HashMap::new(),
            queue: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Insert `source` under `name`, returning its handle.
    ///
    /// If an asset with the same name already exists, its source data is
    /// replaced **in-place** (the same handle is reused and re-queued for
    /// re-upload), and any previously processed result is cleared.
    pub fn insert(&mut self, name: &str, source: T) -> Handle<T> {
        if let Some(&existing) = self.handles.get(name) {
            if let Some(entry) = self.storage.get_mut(existing) {
                entry.source = source;
                entry.processed = None;
                if !self.queue.contains(&existing) {
                    self.queue.push(existing);
                }
                tracing::debug!(
                    "Assets<{}>: replaced source for {:?} ({name}) in-place",
                    std::any::type_name::<T>(),
                    existing
                );
                return Handle::new(existing);
            }
        }
        let handle = self.storage.insert(AssetEntry { source, processed: None });
        self.handles.insert(name.to_string(), handle);
        self.queue.push(handle);
        Handle::new(handle)
    }

    /// Look up the processed (uploaded) asset for `handle`.
    ///
    /// Returns `None` if the handle is null, stale, or the asset has not
    /// finished uploading yet.
    pub fn get(&self, handle: Handle<T>) -> Option<&T::Processed> {
        let id = handle.id;
        if warn_if_null::<T>(id, "get", "did you forget to insert the asset and store the returned handle?") {
            return None;
        }
        match self.storage.get(id) {
            None => {
                tracing::warn!(
                    "Assets<{}>: get() called with a stale handle {:?} — \
                     the asset was likely removed since this handle was obtained",
                    std::any::type_name::<T>(),
                    id
                );
                None
            }
            Some(entry) => {
                if entry.processed.is_none() {
                    tracing::debug!(
                        "Assets<{}>: get() for {:?} returned None — \
                         the asset may still be pending upload",
                        std::any::type_name::<T>(),
                        id
                    );
                }
                entry.processed.as_ref()
            }
        }
    }

    /// Look up the raw source data for `handle`.
    pub fn get_source(&self, handle: Handle<T>) -> Option<&T> {
        let id = handle.id;
        if warn_if_null::<T>(id, "get_source", "did you forget to insert the asset and store the returned handle?") {
            return None;
        }
        let result = self.storage.get(id).map(|e| &e.source);
        if result.is_none() {
            tracing::warn!(
                "Assets<{}>: get_source() called with a stale handle {:?}",
                std::any::type_name::<T>(),
                id
            );
        }
        result
    }

    /// Mutably look up the raw source data for `handle`.
    pub fn get_source_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let id = handle.id;
        if warn_if_null::<T>(id, "get_source_mut", "did you forget to insert the asset and store the returned handle?") {
            return None;
        }
        let result = self.storage.get_mut(id).map(|e| &mut e.source);
        if result.is_none() {
            tracing::warn!(
                "Assets<{}>: get_source_mut() called with a stale handle {:?}",
                std::any::type_name::<T>(),
                id
            );
        }
        result
    }

    /// Returns `true` if `handle` exists and its processed asset is ready.
    /// Never logs — safe to poll speculatively.
    pub fn is_ready(&self, handle: Handle<T>) -> bool {
        self.storage.get(handle.id).is_some_and(|e| e.processed.is_some())
    }

    /// Returns `true` if `handle` currently refers to a present entry.
    /// Never logs — safe to poll speculatively.
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.storage.contains_key(handle.id)
    }

    /// Look up the processed asset by the name it was inserted under.
    /// `None` if the name is unknown or the asset hasn't uploaded yet —
    /// not logged, since "still uploading" is a normal transient state.
    pub fn get_by_name(&self, name: &str) -> Option<&T::Processed> {
        let handle = self.handles.get(name)?;
        self.storage.get(*handle)?.processed.as_ref()
    }

    /// Look up the raw source data by the name it was inserted under.
    pub fn get_source_by_name(&self, name: &str) -> Option<&T> {
        let handle = self.handles.get(name)?;
        Some(&self.storage.get(*handle)?.source)
    }

    /// Look up a handle by name.
    pub fn get_handle_by_name(&self, name: &str) -> Option<Handle<T>> {
        self.handles.get(name).copied().map(Handle::new)
    }

    /// Replace the source data for `handle`, invalidating the processed
    /// result and re-queuing for upload. Returns `false` if the handle is
    /// null or not present.
    pub fn replace(&mut self, handle: Handle<T>, source: T) -> bool {
        let id = handle.id;
        if warn_if_null::<T>(id, "replace", "no-op") {
            return false;
        }
        let Some(entry) = self.storage.get_mut(id) else {
            tracing::warn!(
                "Assets<{}>: replace() called with a stale handle {:?} — no-op",
                std::any::type_name::<T>(),
                id
            );
            return false;
        };
        entry.source = source;
        entry.processed = None;
        if !self.queue.contains(&id) {
            self.queue.push(id);
        }
        tracing::debug!(
            "Assets<{}>: replaced source for {:?}{} via handle",
            std::any::type_name::<T>(),
            id,
            self.name_for_handle(id).map(|n| format!(" ({n})")).unwrap_or_default()
        );
        true
    }

    /// Mark a single asset as dirty so the sync system re-uploads it next
    /// tick, even though its source data has not changed (e.g. a dependency
    /// was recreated). Does nothing if the handle is null or not present.
    pub fn mark_dirty(&mut self, handle: Handle<T>) {
        let id = handle.id;
        if warn_if_null::<T>(id, "mark_dirty", "no-op") {
            return;
        }
        if self.storage.contains_key(id) && !self.queue.contains(&id) {
            self.queue.push(id);
        }
    }

    /// Remove an asset by handle, returning the source value if it existed.
    /// The processed asset is also discarded.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let id = handle.id;
        if warn_if_null::<T>(id, "remove", "no-op") {
            return None;
        }
        let entry = self.storage.remove(id)?;
        self.handles.retain(|_, h| *h != id);
        self.queue.retain(|h| *h != id);
        self.removed.push(id);
        Some(entry.source)
    }

    /// Remove an asset by name, returning the source value if it existed.
    pub fn remove_by_name(&mut self, name: &str) -> Option<T> {
        let id = self.handles.remove(name)?;
        self.queue.retain(|h| *h != id);
        self.removed.push(id);
        Some(self.storage.remove(id)?.source)
    }

    /// Iterate over all entries that have a processed result ready.
    pub fn iter(&self) -> impl Iterator<Item = (RawAssetHandle, &T::Processed)> {
        self.storage
            .iter()
            .filter_map(|(h, e)| e.processed.as_ref().map(|p| (h, p)))
    }

    /// Iterate over all source entries regardless of upload state.
    pub fn iter_source(&self) -> impl Iterator<Item = (RawAssetHandle, &T)> {
        self.storage.iter().map(|(h, e)| (h, &e.source))
    }

    /// Iterate over `(name, handle)` pairs for every named asset.
    pub fn names(&self) -> impl Iterator<Item = (&str, RawAssetHandle)> {
        self.handles.iter().map(|(name, &handle)| (name.as_str(), handle))
    }

    // --- sync-system internals (pub(crate)) ---

    /// Look up the source for `handle` without logging on a miss.
    ///
    /// Used by the sync system, which legitimately encounters handles
    /// removed between being queued dirty and sync running.
    pub(crate) fn get_source_quiet(&self, handle: RawAssetHandle) -> Option<&T> {
        self.storage.get(handle).map(|e| &e.source)
    }

    /// Write the processed result for `handle` back into the entry.
    pub(crate) fn set_processed(&mut self, handle: RawAssetHandle, processed: T::Processed) {
        if let Some(entry) = self.storage.get_mut(handle) {
            entry.processed = Some(processed);
        }
    }

    /// Drain and return all handles currently in the dirty queue.
    pub(crate) fn take_dirty(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.queue)
    }

    /// Drain and return all handles removed since the last call.
    pub(crate) fn take_removed(&mut self) -> Vec<RawAssetHandle> {
        std::mem::take(&mut self.removed)
    }

    /// Push `handles` back onto the dirty queue so they are retried next tick.
    pub(crate) fn requeue(&mut self, handles: Vec<RawAssetHandle>) {
        self.queue.extend(handles);
    }

    /// Returns `true` if the dirty queue is empty.
    pub(crate) fn dirty_is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns the number of handles currently in the dirty queue.
    pub(crate) fn dirty_len(&self) -> usize {
        self.queue.len()
    }

    /// Reverse lookup: the name `handle` was inserted under, if any.
    /// O(n) scan — for diagnostics/logging only, not a hot path.
    pub(crate) fn name_for_handle(&self, handle: RawAssetHandle) -> Option<&str> {
        self.handles
            .iter()
            .find(|(_, h)| **h == handle)
            .map(|(name, _)| name.as_str())
    }
}
