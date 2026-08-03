use std::collections::HashMap;

use crate::{
    app::SystemStage,
    assets::{
        deps::Dependencies,
        storage::{Assets, ProcessedAssets, RawAssetHandle},
        upload::Asset,
    },
    ecs::{
        plugin::Plugin,
        resources::Resources,
        system::{Local, Res, ResMut},
    },
};

/// Ticks a pending asset or a system blocked on `backend`/`Deps` can retry
/// before the pipeline escalates from a quiet `debug!`/`trace!` to a
/// `warn!`. Long enough that a legitimately slow dependency chain (a
/// `LazyResource` waiting on a device, a multi-hop asset dependency) doesn't
/// trip it on every run; short enough that something genuinely stuck
/// (`upload`/`construct` unconditionally returning `None`, a `Deps`
/// resource nothing will ever provide) doesn't stay invisible for minutes.
/// Not a hard limit — retries continue past this, just louder, and repeat
/// every `STUCK_AFTER_TICKS` after the first warning instead of going quiet
/// again.
const STUCK_AFTER_TICKS: u32 = 300;

/// `true` on the tick a stuck-ness warning should fire: the first time
/// `ticks` crosses the threshold, then again every `STUCK_AFTER_TICKS`
/// ticks after that (rather than either spamming every tick past the
/// threshold, or warning exactly once and going silent again).
fn should_warn_stuck(ticks: u32) -> bool {
    ticks >= STUCK_AFTER_TICKS && ticks.is_multiple_of(STUCK_AFTER_TICKS)
}

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
    /// Create the plugin. See the type-level docs for what registering it does.
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
        app.provides::<ProcessedAssets<T>>();
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
    mut blocked_ticks: Local<u32>,
    mut pending_ticks: Local<HashMap<RawAssetHandle, u32>>,
    world: &hecs::World,
    resources: &Resources,
) where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    let Some(backend) = backend else {
        log_waiting::<B, T>(&cpu, "backend", &mut blocked_ticks);
        return;
    };
    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<B, T>(&cpu, "dependencies", &mut blocked_ticks);
        return;
    };
    *blocked_ticks = 0;

    for handle in cpu.take_removed() {
        processed.remove(handle);
        pending_ticks.remove(&handle);
    }

    let mut still_pending = Vec::new();

    for handle in cpu.take_dirty() {
        let Some(source) = cpu.get_quiet(handle) else {
            // Asset was inserted then removed before sync ran — nothing to upload.
            tracing::debug!(
                "{}: handle {:?} was in the dirty queue but the source asset is already gone \
                 (inserted and removed in the same tick?)",
                std::any::type_name::<T>(),
                handle
            );
            pending_ticks.remove(&handle);
            continue;
        };
        match T::upload(source, &backend, &deps) {
            Some(value) => {
                if let Some(name) = cpu.name_for_handle(handle) {
                    processed.names.insert(name.to_string(), handle);
                }
                tracing::debug!(
                    "{}: uploaded {:?}{}",
                    std::any::type_name::<T>(),
                    handle,
                    cpu.name_for_handle(handle)
                        .map(|n| format!(" ({n})"))
                        .unwrap_or_default()
                );
                processed.insert(handle, value);
                pending_ticks.remove(&handle);
            }
            None => {
                let ticks = pending_ticks.entry(handle).or_insert(0);
                *ticks += 1;
                if should_warn_stuck(*ticks) {
                    tracing::warn!(
                        "{}: {:?}{} has not uploaded after {} ticks — upload() may be \
                         unconditionally returning None, or a Deps resource it needs is never \
                         actually going to appear. Still retrying every tick.",
                        std::any::type_name::<T>(),
                        handle,
                        cpu.name_for_handle(handle)
                            .map(|n| format!(" ({n})"))
                            .unwrap_or_default(),
                        *ticks
                    );
                } else {
                    tracing::debug!(
                        "{}: {:?} upload returned None — a required dependency is not yet ready, \
                         requeued for next tick",
                        std::any::type_name::<T>(),
                        handle
                    );
                }
                still_pending.push(handle);
            }
        }
    }

    if !still_pending.is_empty() {
        tracing::debug!(
            "{}: {} handle(s) still pending upload (waiting on dependencies)",
            std::any::type_name::<T>(),
            still_pending.len()
        );
    }

    cpu.requeue(still_pending);
}

fn log_waiting<D, T>(cpu: &Assets<T::Source>, what: &str, blocked_ticks: &mut u32)
where
    D: 'static + Send + Sync,
    T: Asset<D>,
{
    if cpu.dirty_is_empty() {
        *blocked_ticks = 0;
        return;
    }

    *blocked_ticks += 1;
    if should_warn_stuck(*blocked_ticks) {
        tracing::warn!(
            "{}: {} asset(s) have been queued for {} ticks, still waiting on {what} before \
             upload can begin — if {what} is never going to appear, this pipeline will wait \
             forever.",
            std::any::type_name::<T>(),
            cpu.dirty_len(),
            *blocked_ticks,
        );
    } else {
        tracing::debug!(
            "{}: {} asset(s) queued but waiting on {what} before upload can begin",
            std::any::type_name::<T>(),
            cpu.dirty_len()
        );
    }
}
