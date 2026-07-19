//! Lazy resources — singleton GPU resources that are constructed once,
//! on-demand, as soon as their backend and dependencies become available,
//! but (unlike [`Asset`]) are never tracked by a [`Handle`] or stored in a
//! pool, because there is only ever one of them.
//!
//! Use this for things like: a shared bind group layout, a camera's GPU
//! buffer, a depth texture — anything that (a) needs a device/backend and
//! possibly other resources to exist before it can be built, and (b) has
//! exactly one instance for the whole app, accessed directly via `Res<T>`
//! rather than through a handle.
//!
//! If you find yourself wanting more than one instance of something (e.g.
//! multiple cameras, multiple independently-loaded textures), that's a
//! sign you actually want [`Asset`] + [`Handle`], not [`LazyResource`].

use crate::{
    app::SystemStage,
    assets::deps::Dependencies,
    ecs::plugin::Plugin,
    ecs::resources::Resources,
    ecs::system::{Commands, Res},
};

/// A resource that is constructed once, lazily, as soon as its device and
/// dependencies are available — and never rebuilt or reconstructed after
/// that (unless you explicitly remove it yourself).
///
/// Unlike [`Asset`], there is no `Source` and no `Handle` — a
/// `LazyResource` has nothing authored to parse from; it's pure
/// construction from a device plus whatever else it depends on.
pub trait LazyResource<B>: 'static + Send + Sync + Sized {
    /// Other resources this singleton needs before it can be built.
    /// Use `()` if none are needed.
    type Deps<'a>: Dependencies<'a>;

    /// Attempt to construct this singleton. Return `None` if construction
    /// can't succeed yet for a reason not already covered by `Deps`
    /// readiness (e.g. a transient condition) — the plugin will retry
    /// next tick.
    fn construct<'a>(backend: &B, deps: &Self::Deps<'a>) -> Option<Self>;
}

/// Registers the system that lazily constructs `T` once `B` and `T::Deps`
/// are available, and never again afterward.
pub struct LazyResourcePlugin<B, T: LazyResource<B>> {
    _marker: std::marker::PhantomData<(B, T)>,
}

impl<B, T: LazyResource<B>> LazyResourcePlugin<B, T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B, T> Plugin for LazyResourcePlugin<B, T>
where
    B: 'static + Send + Sync,
    T: LazyResource<B>,
{
    fn build(&self, app: &mut crate::prelude::App) {
        app.add_system(SystemStage::AssetSyncDeps, construct_resource::<B, T>);
    }
}

fn construct_resource<B, T>(
    mut commands: Commands,
    backend: Option<Res<B>>,
    existing: Option<Res<T>>,
    world: &hecs::World,
    resources: &Resources,
) where
    B: 'static + Send + Sync,
    T: LazyResource<B>,
{
    // Already built — nothing to do, forever.
    if existing.is_some() {
        return;
    }

    let Some(backend) = backend else {
        tracing::trace!(
            "{}: waiting on backend to construct resource",
            std::any::type_name::<T>()
        );
        return;
    };

    let Some(deps) = T::Deps::try_gather(world, resources) else {
        tracing::trace!(
            "{}: waiting on dependencies to construct resource",
            std::any::type_name::<T>()
        );
        return;
    };

    match T::construct(&backend, &deps) {
        Some(value) => {
            commands.insert_resource(value);
        }
        None => {
            tracing::trace!(
                "{}: construct() returned None, will retry next tick",
                std::any::type_name::<T>()
            );
        }
    }
}
