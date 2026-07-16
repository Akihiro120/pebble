use crate::assets::storage::RawAssetHandle;

pub struct Handle<T> {
    pub id: RawAssetHandle,
    _marker: std::marker::PhantomData<T>,
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
