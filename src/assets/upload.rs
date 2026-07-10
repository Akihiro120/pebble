use crate::{assets::dependent_asset_plugin::Dependencies, prelude::Backend};

pub trait DeviceUpload<B: Backend>: 'static + Send + Sync + Sized {
    type Source: 'static + Send + Sync;
    type Deps<'a>: Dependencies<'a>;

    fn upload<'a>(source: &Self::Source, backend: &B, deps: &Self::Deps<'a>) -> Self;
}
