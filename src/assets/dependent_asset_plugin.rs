use crate::{resources::Resources, system::Res};

pub trait Dependencies<'a>: Sized {
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self>;
}

impl<'a> Dependencies<'a> for () {
    fn try_gather(_world: &'a hecs::World, _resources: &'a Resources) -> Option<Self> {
        Some(())
    }
}

impl<'a, A: 'static + Send + Sync> Dependencies<'a> for Res<'a, A> {
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self> {
        if !resources.has_resource::<A>(world) {
            return None;
        }

        Some(Res {
            data: resources.get_resource(world),
        })
    }
}

impl<'a, A, B> Dependencies<'a> for (Res<'a, A>, Res<'a, B>)
where
    A: 'static + Send + Sync,
    B: 'static + Send + Sync,
{
    fn try_gather(world: &'a hecs::World, resources: &'a Resources) -> Option<Self> {
        if !resources.has_resource::<A>(world) || !resources.has_resource::<B>(world) {
            return None;
        }

        Some((
            Res {
                data: resources.get_resource(world),
            },
            Res {
                data: resources.get_resource(world),
            },
        ))
    }
}

// macro_rules! impl_dependent_asset {
//     ($plugin:ident, $trait_name:ident, $sync_fn:ident, $($dep_ty:ident : $dep_var:ident),+) => {
//         pub trait $trait_name<B, $($dep_ty),+>: 'static + Send + Sync + Sized
//         where $($dep_ty: 'static + Send + Sync),+
//         {
//             type Source: 'static + Send + Sync;
//             fn upload(source: &Self::Source, backend: &B, $($dep_var: &$dep_ty),+) -> Self;
//         }
//         pub struct $plugin<B, T, $($dep_ty),+>(std::marker::PhantomData<(B, T, $($dep_ty),+)>);
//         impl<B, T, $($dep_ty),+> $plugin<B, T, $($dep_ty),+> {
//             pub fn new() -> Self { Self(std::marker::PhantomData) }
//         }
//         impl<B, T, $($dep_ty),+> Plugin for $plugin<B, T, $($dep_ty),+>
//         where
//             B: 'static + Send + Sync,
//             $($dep_ty: 'static + Send + Sync,)+
//             T: $trait_name<B, $($dep_ty),+>,
//         {
//             fn build(&self, app: &mut crate::app::App) {
//                 app.try_insert_resource(Assets::<T::Source>::new());
//                 app.try_insert_resource(GPUAssets::<T>::new());
//                 app.add_system(SystemStage::AssetSyncDeps, $sync_fn::<B, T, $($dep_ty),+>);
//             }
//         }
//         #[allow(clippy::too_many_arguments)]
//         fn $sync_fn<B, T, $($dep_ty),+>(
//             mut cpu: ResMut<Assets<T::Source>>,
//             mut gpu: ResMut<GPUAssets<T>>,
//             backend: Option<Res<B>>,
//             $($dep_var: Option<Res<$dep_ty>>),+
//         ) where
//             B: 'static + Send + Sync,
//             $($dep_ty: 'static + Send + Sync,)+
//             T: $trait_name<B, $($dep_ty),+>,
//         {
//             let Some(backend) = backend else { return };
//             $(let Some($dep_var) = $dep_var else { return };)+
//             for handle in cpu.take_dirty() {
//                 if let Some(source) = cpu.get(handle) {
//                     gpu.insert(handle, T::upload(source, &backend, $(&$dep_var),+));
//                 } else {
//                     tracing::warn!("Dirty asset handle removed before GPU sync");
//                 }
//             }
//         }
//     };
// }
//
// impl_dependent_asset!(
//     DependentAssetPlugin1, DependentUpload1, sync_dependent1,
//     Dep1: dep1
// );
// impl_dependent_asset!(
//     DependentAssetPlugin2, DependentUpload2, sync_dependent2,
//     Dep1: dep1, Dep2: dep2
// );
// impl_dependent_asset!(
//     DependentAssetPlugin3, DependentUpload3, sync_dependent3,
//     Dep1: dep1, Dep2: dep2, Dep3: dep3
// );
// impl_dependent_asset!(
//     DependentAssetPlugin4, DependentUpload4, sync_dependent4,
//     Dep1: dep1, Dep2: dep2, Dep3: dep3, Dep4: dep4
// );
