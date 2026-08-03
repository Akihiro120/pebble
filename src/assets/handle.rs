use crate::assets::storage::RawAssetHandle;

/// A typed wrapper around a [`RawAssetHandle`].
///
/// `Handle<T>` is cheap to copy and store. It does not keep the underlying
/// asset alive — the asset lives in [`Assets<T::Source>`](crate::assets::storage::Assets)
/// and can be removed independently.
///
/// `Handle<T>::default()` is the null handle — the same sentinel every
/// lookup method already treats as "definitely not present" (see
/// [`Assets::get`](crate::assets::storage::Assets::get)'s null-handle
/// check), useful as a placeholder before an asset has been inserted.
pub struct Handle<T> {
    pub id: RawAssetHandle,
    // `fn() -> T` rather than `T` directly: this handle never actually
    // stores a `T`, only a small `Copy` key, so it should be `Send`/`Sync`
    // regardless of whether `T` is — `PhantomData<T>` would incorrectly tie
    // those auto-traits (and variance) to `T`.
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Create a new typed handle from a raw slot-map key.
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
