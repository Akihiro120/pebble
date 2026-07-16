use crate::assets::deps::Dependencies;

pub trait Asset<B>: 'static + Send + Sync + Sized {
    type Source: 'static + Send + Sync;
    type Deps<'a>: Dependencies<'a>;

    fn upload<'a>(source: &Self::Source, backend: &B, deps: &Self::Deps<'a>) -> Self;
}
