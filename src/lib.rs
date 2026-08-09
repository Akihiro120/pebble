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
//! Assets flow through a unified store: CPU-side source data and the
//! uploaded result both live in [`assets::storage::Assets`] per entry.
//! An [`assets::plugin::AssetPlugin`] calls [`assets::upload::Asset::upload`]
//! each tick; rendering systems then use [`Assets::get`] to read the
//! processed result. [`assets::handle::Handle`] is the typed key.
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
//!
//! [`time::TimePlugin`] ticks [`time::Time`] (delta/elapsed seconds, fps)
//! once per frame in `PreUpdate` — backend-agnostic, so it works the same
//! whether or not a graphics backend is even in use. [`gamepad::GamepadPlugin`]/
//! [`audio::AudioPlugin`] are built in the same way, alongside it.

pub mod app;
pub mod assets;
pub mod audio;
pub mod ecs;
pub mod gamepad;
pub mod prelude;
pub mod rendering;
pub mod threading;
pub mod time;
pub mod wgpu;
