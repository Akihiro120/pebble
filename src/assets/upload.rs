use crate::assets::deps::Dependencies;

pub trait AssetSource: 'static {
    type Processed: 'static;
}

pub trait Asset<B>: AssetSource {
    type Deps<'a>: Dependencies<'a>;

    fn upload<'a>(&self, backend: &B, deps: &Self::Deps<'a>) -> Option<Self::Processed>;
}
