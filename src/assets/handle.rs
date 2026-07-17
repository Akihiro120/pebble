use crate::assets::storage::RawAssetHandle;

/// A typed wrapper around a [`RawAssetHandle`].
///
/// `Handle<T>` is cheap to copy and store. It does not keep the underlying
/// asset alive — the asset lives in [`Assets<T::Source>`](crate::assets::storage::Assets)
/// and can be removed independently.
pub struct Handle<T> {
    pub id: RawAssetHandle,
    _marker: std::marker::PhantomData<T>,
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
