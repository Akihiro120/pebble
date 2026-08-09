//! The CPU → GPU asset pipeline.
//!
//! [`storage::Assets<T>`] holds both raw source data and uploaded results,
//! inserted and looked up by [`handle::Handle<T>`]. [`plugin::AssetPlugin`]
//! drains newly-inserted or -changed entries each tick and calls
//! [`upload::Asset::upload`] to convert them, waiting on whatever
//! [`deps::Dependencies`] the target type declares. The result is written
//! back into the same [`Assets<T>`] entry — use [`Assets::get`] to retrieve
//! it for rendering, and [`Assets::get_source`] to access the raw data.
//!
//! [`singleton_asset`] covers the adjacent but distinct case of a single
//! lazily-constructed resource (no name, no CPU source, exactly one
//! instance) rather than a named collection. [`required`] is the shared
//! "this resource arrives asynchronously, wait rather than error" mechanism
//! both `AssetPlugin` and `LazyResourcePlugin` register with.

pub mod deps;
pub mod handle;
pub mod plugin;
pub mod storage;
pub mod upload;
