use crate::assets::deps::Dependencies;

pub trait DeviceUpload<D>: 'static + Send + Sync + Sized {
    type Source: 'static + Send + Sync;
    type Deps<'a>: Dependencies<'a>;

    fn upload<'a>(source: &Self::Source, device: &D, deps: &Self::Deps<'a>) -> Self;
}
