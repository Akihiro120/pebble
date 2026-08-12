use crate::assets::deps::Dependencies;

/// Declares what a CPU-side asset type turns into once uploaded. Usually
/// implemented together with [`Asset<B>`] — see the [`asset!`](crate::asset)
/// macro for the terser way to write both at once.
pub trait AssetSource: 'static {
    type Processed: 'static;
}

/// Describes how to turn a CPU-side source into its GPU-side (or otherwise
/// processed) form using backend `B`. `Deps<'a>` names any other resources
/// the upload needs; if they, or `B` itself, aren't available yet,
/// `upload` returning `None` just retries next tick — no manual ordering.
pub trait Asset<B>: AssetSource {
    type Deps<'a>: Dependencies<'a>;

    fn upload<'a>(&self, backend: &B, deps: &Self::Deps<'a>) -> Option<Self::Processed>;
}

/// Expands to the same [`AssetSource`]/[`Asset<B>`] impls you'd write by
/// hand — a pure syntax transform, so every built-in asset type keeps using
/// the trait impls directly. Only exists to make a *new*, user-defined
/// asset type cheaper to write:
///
/// ```ignore
/// asset!(MyThing => GPUMyThing, |self, backend: &Backend| {
///     Some(GPUMyThing { .. })
/// });
///
/// asset!(MyThing => GPUMyThing, deps: [SomePool], |self, backend: &Backend, deps| {
///     Some(GPUMyThing { .. })
/// });
/// ```
///
/// `deps: [..]` takes bare types — wrapped in `Read<'a, _>` for one, or a
/// tuple of `Read<'a, _>` for several (matching what [`Dependencies`] supports).
#[macro_export]
macro_rules! asset {
    ($ty:ty => $processed:ty, |$self:ident, $backend:ident : &$backend_ty:ty| $body:block) => {
        impl $crate::assets::upload::AssetSource for $ty {
            type Processed = $processed;
        }
        impl $crate::assets::upload::Asset<$backend_ty> for $ty {
            type Deps<'a> = ();

            fn upload<'a>(&$self, $backend: &$backend_ty, _deps: &()) -> Option<$processed> $body
        }
    };

    ($ty:ty => $processed:ty, deps: [$dep:ty], |$self:ident, $backend:ident : &$backend_ty:ty, $deps:ident| $body:block) => {
        impl $crate::assets::upload::AssetSource for $ty {
            type Processed = $processed;
        }
        impl $crate::assets::upload::Asset<$backend_ty> for $ty {
            type Deps<'a> = $crate::ecs::resources::Read<'a, $dep>;

            fn upload<'a>(&$self, $backend: &$backend_ty, $deps: &Self::Deps<'a>) -> Option<$processed> $body
        }
    };

    ($ty:ty => $processed:ty, deps: [$($dep:ty),+ $(,)?], |$self:ident, $backend:ident : &$backend_ty:ty, $deps:ident| $body:block) => {
        impl $crate::assets::upload::AssetSource for $ty {
            type Processed = $processed;
        }
        impl $crate::assets::upload::Asset<$backend_ty> for $ty {
            type Deps<'a> = ($($crate::ecs::resources::Read<'a, $dep>,)+);

            fn upload<'a>(&$self, $backend: &$backend_ty, $deps: &Self::Deps<'a>) -> Option<$processed> $body
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        assets::{plugin::sync_assets, storage::Assets},
        ecs::{resources::Resources, schedule::Schedule},
    };

    struct TestBackend(i32);
    struct DepA(i32);
    struct DepB(i32);

    struct NoDepsAsset(i32);
    crate::asset!(NoDepsAsset => i32, |self, backend: &TestBackend| {
        Some(self.0 * backend.0)
    });

    struct OneDepAsset(i32);
    crate::asset!(OneDepAsset => i32, deps: [DepA], |self, backend: &TestBackend, deps| {
        Some(self.0 * backend.0 * deps.0)
    });

    struct TwoDepAsset(i32);
    crate::asset!(TwoDepAsset => i32, deps: [DepA, DepB], |self, backend: &TestBackend, deps| {
        Some(self.0 * backend.0 * deps.0.0 * deps.1.0)
    });

    #[test]
    fn no_deps_asset_uploads_once_the_backend_exists() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Assets::<NoDepsAsset>::new());

        let handle = resources.get_mut::<Assets<NoDepsAsset>>().insert("test", NoDepsAsset(3));

        let mut schedule = Schedule::default();
        schedule.add_system(sync_assets::<TestBackend, NoDepsAsset>);

        schedule.run(&mut world, &mut resources);
        assert!(!resources.get::<Assets<NoDepsAsset>>().is_ready(handle));

        resources.insert(TestBackend(2));
        schedule.run(&mut world, &mut resources);
        assert_eq!(*resources.get::<Assets<NoDepsAsset>>().get(handle).unwrap(), 6);
    }

    #[test]
    fn one_dep_asset_waits_for_its_dependency() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Assets::<OneDepAsset>::new());
        resources.insert(TestBackend(2));

        let handle = resources.get_mut::<Assets<OneDepAsset>>().insert("test", OneDepAsset(3));

        let mut schedule = Schedule::default();
        schedule.add_system(sync_assets::<TestBackend, OneDepAsset>);

        schedule.run(&mut world, &mut resources);
        assert!(!resources.get::<Assets<OneDepAsset>>().is_ready(handle));

        resources.insert(DepA(5));
        schedule.run(&mut world, &mut resources);
        assert_eq!(*resources.get::<Assets<OneDepAsset>>().get(handle).unwrap(), 30);
    }

    #[test]
    fn two_dep_asset_waits_for_both_dependencies() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Assets::<TwoDepAsset>::new());
        resources.insert(TestBackend(2));
        resources.insert(DepA(5));

        let handle = resources.get_mut::<Assets<TwoDepAsset>>().insert("test", TwoDepAsset(3));

        let mut schedule = Schedule::default();
        schedule.add_system(sync_assets::<TestBackend, TwoDepAsset>);

        schedule.run(&mut world, &mut resources);
        assert!(!resources.get::<Assets<TwoDepAsset>>().is_ready(handle));

        resources.insert(DepB(7));
        schedule.run(&mut world, &mut resources);
        assert_eq!(*resources.get::<Assets<TwoDepAsset>>().get(handle).unwrap(), 210);
    }
}
