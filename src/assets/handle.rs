use crate::assets::storage::RawAssetHandle;

/// A lightweight reference to an entry in `Assets<T>`. `Copy`, `Eq`,
/// `Hash` — safe to store in a component or use as a map key. A stale
/// handle (its entry removed) just yields `None` from `Assets<T>`'s
/// lookups, never panics.
pub struct Handle<T> {
    pub id: RawAssetHandle,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub fn new(id: RawAssetHandle) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        Self::new(RawAssetHandle::default())
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<T> Eq for Handle<T> {}

impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Handle").field(&self.id).finish()
    }
}
