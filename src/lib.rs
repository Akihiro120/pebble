//! A modular, ECS-style graphics/app framework.
//!
//! Start with [`prelude`] — `use pebble::prelude::*;` pulls in the types
//! you need for almost everything below.
//!
//! The lifecycle: build an [`app::App`], register [`ecs::plugin::Plugin`]s
//! (a windowing backend, a graphics backend, your own gameplay plugins),
//! register systems against an [`app::SystemStage`], then [`app::App::build`]
//! and [`app::App::run`]. Systems are plain functions whose parameters
//! (`Res`, `ResMut`, `Query`, `Commands`, ...) declare what ECS state they
//! touch — see [`ecs::system`] for the full set.
//!
//! Assets flow one direction: CPU-side source data goes into
//! [`assets::storage::Assets`], an [`assets::plugin::AssetPlugin`] uploads
//! it to a backend via [`assets::upload::Asset::upload`], and the result
//! lands in [`assets::storage::ProcessedAssets`] for rendering systems to
//! read. [`assets::handle::Handle`] is the typed handle threaded through
//! all three.
//!
//! [`rendering`] defines the backend-agnostic contract ([`rendering::backend::Backend`],
//! [`rendering::window::WindowProvider`]); [`wgpu`] is a ready-to-use wgpu
//! implementation of it, plus a higher-level descriptor-based material/mesh/
//! texture layer (see [`wgpu::backend::WGPUPlugin`]) that needs far less
//! boilerplate than implementing `Backend`/`Asset` by hand.
//!
//! [`threading::BackgroundTasks`] offloads CPU-bound work to a worker pool;
//! [`ecs::events`] and [`ecs::system::AsyncExt`] build on it for ECS events
//! and fire-and-forget async systems.

pub mod app;
pub mod assets;
pub mod ecs;
pub mod prelude;
pub mod rendering;
pub mod threading;
pub mod wgpu;
