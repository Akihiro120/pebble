use crate::prelude::Backend;

pub trait DeviceUpload<B: Backend>: 'static + Send + Sync + Sized {
    type Source: 'static + Send + Sync;

    fn upload(source: &Self::Source, backend: &B) -> Self;
}
