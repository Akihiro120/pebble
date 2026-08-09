use std::collections::HashMap;

use crate::{
    app::SystemStage,
    assets::{
        deps::Dependencies,
        storage::{Assets, RawAssetHandle},
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
/// `warn!`. Long enough that a legitimately slow dependency chain doesn't
/// trip it on every run; short enough that something genuinely stuck doesn't
/// stay invisible for minutes. Not a hard limit — retries continue past
/// this, just louder, and repeat every `STUCK_AFTER_TICKS`.
const STUCK_AFTER_TICKS: u32 = 300;

fn should_warn_stuck(ticks: u32) -> bool {
    ticks >= STUCK_AFTER_TICKS && ticks.is_multiple_of(STUCK_AFTER_TICKS)
}

/// Plugin that drives the source → processed conversion pipeline for a
/// single asset type `T`.
///
/// `B` is the *backend* passed to [`Asset::upload`] and is intentionally
/// generic — it need not be a GPU backend.
///
/// Registering `AssetPlugin::<B, T>::new()` will:
/// - Insert an [`Assets<T>`] resource holding both source and processed data.
/// - Add a system on [`SystemStage::AssetSync`] that flushes the dirty
///   queue each tick, calling [`Asset::upload`] for every pending entry.
///
/// The sync system waits silently until both `B` and all of `T`'s
/// [`Dependencies`] are present as resources before processing any uploads.
pub struct AssetPlugin<B, T: Asset<B>> {
    _marker: std::marker::PhantomData<(B, T)>,
}

impl<B, T: Asset<B>> AssetPlugin<B, T> {
    pub fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

impl<B, T> Plugin for AssetPlugin<B, T>
where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    fn build(&self, app: &mut crate::app::App) {
        app.try_insert_resource(Assets::<T>::new());
        app.add_system(SystemStage::AssetSync, sync_assets::<B, T>);
    }
}

/// Per-tick system: flush the dirty queue and convert pending assets.
///
/// Skips processing if `B` or any dependency is not yet available as a
/// resource. Assets whose [`Asset::upload`] returns `None` are re-queued
/// for the next tick.
fn sync_assets<B, T>(
    mut assets: ResMut<Assets<T>>,
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
        log_waiting::<B, T>(&assets, "backend", &mut blocked_ticks);
        return;
    };
    let Some(deps) = T::Deps::try_gather(world, resources) else {
        log_waiting::<B, T>(&assets, "dependencies", &mut blocked_ticks);
        return;
    };
    *blocked_ticks = 0;

    for handle in assets.take_removed() {
        pending_ticks.remove(&handle);
    }

    let dirty = assets.take_dirty();
    let mut still_pending = Vec::new();

    // Phase 1: compute upload results while holding an immutable borrow on
    // `assets`. Outer `None` = source gone; `Some(None)` = upload pending;
    // `Some(Some(v))` = ready. Collected into an owned vec so all borrows
    // are released before phase 2 writes back.
    let results: Vec<(RawAssetHandle, Option<String>, Option<Option<T::Processed>>)> = dirty
        .iter()
        .map(|&handle| match assets.get_source_quiet(handle) {
            None => (handle, None, None),
            Some(source) => {
                let name = assets.name_for_handle(handle).map(String::from);
                let outcome = source.upload(&backend, &deps);
                (handle, name, Some(outcome))
            }
        })
        .collect();

    // Phase 2: apply results with mutable borrows on `assets`.
    for (handle, name, outcome) in results {
        let label = name.as_deref().map(|n| format!(" ({n})")).unwrap_or_default();
        match outcome {
            None => {
                tracing::debug!(
                    "{}: handle {:?} was in the dirty queue but the source asset is already \
                     gone (inserted and removed in the same tick?)",
                    std::any::type_name::<T>(),
                    handle
                );
                pending_ticks.remove(&handle);
            }
            Some(None) => {
                let ticks = pending_ticks.entry(handle).or_insert(0);
                *ticks += 1;
                if should_warn_stuck(*ticks) {
                    tracing::warn!(
                        "{}: {:?}{} has not uploaded after {} ticks — upload() may be \
                         unconditionally returning None, or a Deps resource it needs is never \
                         actually going to appear. Still retrying every tick.",
                        std::any::type_name::<T>(),
                        handle,
                        label,
                        *ticks
                    );
                } else {
                    tracing::debug!(
                        "{}: {:?}{} upload returned None — a required dependency is not yet \
                         ready, requeued for next tick",
                        std::any::type_name::<T>(),
                        handle,
                        label,
                    );
                }
                still_pending.push(handle);
            }
            Some(Some(value)) => {
                assets.set_processed(handle, value);
                pending_ticks.remove(&handle);
                tracing::debug!(
                    "{}: uploaded {:?}{}",
                    std::any::type_name::<T>(),
                    handle,
                    label
                );
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

    assets.requeue(still_pending);
}

fn log_waiting<B, T>(assets: &Assets<T>, what: &str, blocked_ticks: &mut u32)
where
    B: 'static + Send + Sync,
    T: Asset<B>,
{
    if assets.dirty_is_empty() {
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
            assets.dirty_len(),
            *blocked_ticks,
        );
    } else {
        tracing::debug!(
            "{}: {} asset(s) queued but waiting on {what} before upload can begin",
            std::any::type_name::<T>(),
            assets.dirty_len()
        );
    }
}
