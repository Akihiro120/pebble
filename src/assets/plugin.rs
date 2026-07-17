use crate::{
    app::SystemStage,
    assets::{
        deps::Dependencies,
        storage::{Assets, ProcessedAssets},
        upload::Asset,
    },
    ecs::{
        plugin::Plugin,
        resources::Resources,
        system::{Res, ResMut},
    },
};

/// Plugin that drives the source → processed conversion pipeline for a single
/// asset type `T`.
///
/// `B` is the *backend* passed to [`Asset::upload`] and is intentionally
/// generic — it need not be a GPU backend:
/// - **GPU assets**: `B` = your graphics backend (e.g. wgpu `Device`).
/// - **CPU-only / audio / other**: `B = ()` or any other service type.
///
/// Registering `AssetPlugin::<B, T>::new()` will:
/// - Insert an [`Assets<T::Source>`] resource for raw source assets.
/// - Insert a [`ProcessedAssets<T>`] resource for the converted results.
/// - Add a system on [`SystemStage::AssetSync`] that flushes the dirty queue
///   each tick, calling [`Asset::upload`] for every pending entry.
///
/// The sync system waits silently until both `B` and all of `T`'s
/// [`Dependencies`] are present as resources before processing any uploads.
pub struct AssetPlugin<B, T: Asset<B>> {
    _marker: std::marker::PhantomData<(B, T)>,
}

impl<B, T: Asset<B>> AssetPlugin<B, T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<B, T> Plugin for AssetPlugin<B, T>
where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    fn build(&self, app: &mut crate::app::App) {
        app.try_insert_resource(Assets::<T::Source>::new());
        app.try_insert_resource(ProcessedAssets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_assets::<B, T>);
        app.required.provides::<ProcessedAssets<T>>();
    }
}

/// Per-tick system: flush the dirty queue and convert pending assets.
///
/// Skips processing if `B` or any dependency is not yet available as a
/// resource. Assets whose [`Asset::upload`] returns `None` are re-queued for
/// the next tick.
fn sync_assets<B, T>(
    mut cpu: ResMut<Assets<T::Source>>,
    mut processed: ResMut<ProcessedAssets<T>>,
    backend: Option<Res<B>>,
    world: &hecs::World,
    resources: &Resources,
) where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    let Some(backend) = backend else {
        log_waiting::<B, T>(&cpu, "backend");
        return;
    };
    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<B, T>(&cpu, "dependencies");
        return;
    };

    let mut still_pending = Vec::new();

    for handle in cpu.take_dirty() {
        let Some(source) = cpu.get(handle) else {
            continue;
        };
        match T::upload(source, &backend, &deps) {
            Some(value) => {
                processed.insert(handle, value);
            }
            None => {
                tracing::trace!(
                    "{}: {handle:?} not ready yet (dependency exists but entry missing), requeued",
                    std::any::type_name::<T>()
                );
                still_pending.push(handle);
            }
        }
    }

    if !still_pending.is_empty() {
        tracing::trace!(
            "{}: {} handle(s) requeued this tick",
            std::any::type_name::<T>(),
            still_pending.len()
        );
    }

    cpu.requeue(still_pending);
}

fn log_waiting<D, T>(cpu: &Assets<T::Source>, what: &str)
where
    D: 'static + Send + Sync,
    T: Asset<D>,
{
    if !cpu.dirty_is_empty() {
        tracing::trace!(
            "{}: waiting on {what}, {} pending",
            std::any::type_name::<T>(),
            cpu.dirty_len()
        );
    }
}
