use crate::assets::deps::Dependencies;

/// Declares the processed form of a source asset type.
///
/// Implement this alongside [`Asset<B>`] for every source type that is
/// uploaded to a backend. Used as the storage bound for
/// [`Assets<T>`](crate::assets::storage::Assets) — the struct that holds
/// both `T` (source) and `T::Processed` (uploaded result) per entry.
///
/// Kept separate from `Asset<B>` so that `Assets<T>` does not need to know
/// the backend type `B`.
pub trait AssetSource: 'static + Send + Sync {
    /// The uploaded (GPU/processed) form produced by [`Asset::upload`].
    type Processed: 'static + Send + Sync;
}

/// Describes how a source asset of type `Self` is converted into its
/// processed form [`Self::Processed`](AssetSource::Processed), using a
/// backend `B`.
///
/// `B` is intentionally generic — it is not restricted to GPU backends:
/// - **CPU → GPU**: `B` is your graphics backend (e.g. a wgpu device).
/// - **CPU → CPU**: set `B = ()` for pure data transforms.
/// - **Audio / other**: `B` is your audio device or any other service.
///
/// # Associated types
/// - `Processed` (via [`AssetSource`]) — the result stored alongside the
///   source in [`Assets<Self>`](crate::assets::storage::Assets).
/// - `Deps` — zero or more additional resources required during the
///   conversion (see [`Dependencies`]). Use `()` when there are none.
///
/// # Upload lifecycle
/// [`AssetPlugin`](crate::assets::plugin::AssetPlugin) drains the dirty
/// queue from `Assets<Self>` each tick and calls [`upload`] for every
/// pending entry. Returning `None` re-queues the handle for the next tick,
/// allowing conversions to wait on sub-resources that may not be ready yet.
pub trait Asset<B>: AssetSource {
    /// Resources (beyond `B` itself) required to perform the conversion.
    /// Use `()` when there are no extra dependencies.
    type Deps<'a>: Dependencies<'a>;

    /// Convert `self` (the source) into its processed form using `backend`
    /// and `deps`.
    ///
    /// Return `None` to defer the conversion to the next tick (e.g. a
    /// required sub-resource is not yet available).
    fn upload<'a>(&self, backend: &B, deps: &Self::Deps<'a>) -> Option<Self::Processed>;
}
