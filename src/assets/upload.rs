use crate::assets::deps::Dependencies;

/// Describes how a source asset of type `Self::Source` is converted into a
/// processed asset `Self`, optionally using a backend `B`.
///
/// `B` is intentionally generic — it is not restricted to GPU backends:
/// - **CPU → GPU**: `B` is your graphics backend (e.g. a wgpu `Device`).
/// - **CPU → CPU**: set `B = ()` for pure data transforms (e.g. decoding,
///   decompression, or format conversion with no device required).
/// - **Audio / other**: set `B` to your audio device or any other service.
///
/// # Associated types
/// - `Source` — the raw unprocessed data stored in [`Assets<Source>`](crate::assets::storage::Assets).
/// - `Deps` — zero or more additional resources required during the conversion
///   (see [`Dependencies`]). Use `()` when there are no extra dependencies.
///
/// # Upload lifecycle
/// [`AssetPlugin`](crate::assets::plugin::AssetPlugin) drains the dirty queue
/// from `Assets<Source>` each tick and calls [`upload`] for every pending
/// entry. Returning `None` re-queues the handle for the next tick, allowing
/// conversions to wait on sub-resources that may not be ready yet.
pub trait Asset<B>: 'static + Send + Sync + Sized {
    /// The raw source representation stored in [`Assets`](crate::assets::storage::Assets).
    type Source: 'static + Send + Sync;

    /// Resources (beyond `B` itself) required to perform the conversion.
    /// Use `()` when there are no extra dependencies.
    type Deps<'a>: Dependencies<'a>;

    /// Convert `source` into a processed asset using `backend` and `deps`.
    ///
    /// Return `None` to defer the conversion to the next tick (e.g. a required
    /// sub-resource is not yet available).
    fn upload<'a>(source: &Self::Source, backend: &B, deps: &Self::Deps<'a>) -> Option<Self>;
}
