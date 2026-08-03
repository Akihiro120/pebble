//! The CPU → GPU asset pipeline.
//!
//! [`storage::Assets<T>`] holds raw source data, inserted and looked up by
//! [`handle::Handle<T>`]. [`plugin::AssetPlugin`] drains newly-inserted or
//! -changed entries each tick and calls [`upload::Asset::upload`] to
//! convert them, waiting on whatever [`deps::Dependencies`] the target type
//! declares. The result lands in [`storage::ProcessedAssets<T>`] for
//! rendering systems to read.
//!
//! [`singleton_asset`] covers the adjacent but distinct case of a single
//! lazily-constructed resource (no name, no CPU source, exactly one
//! instance) rather than a named collection. [`required`] is the shared
//! "this resource arrives asynchronously, wait rather than error" mechanism
//! both `AssetPlugin` and `LazyResourcePlugin` register with.

pub mod deps;
pub mod handle;
pub mod plugin;
pub mod required;
pub mod singleton_asset;
pub mod storage;
pub mod upload;
