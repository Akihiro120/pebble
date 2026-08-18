//! A low-level Rust game engine: an ECS ([`ecs`]) plus a wgpu-backed
//! renderer ([`graphics`]) and asset pipeline ([`assets`]), wired together
//! by [`app::App`]. No built-in high-level systems (no scene graph, no
//! physics) — pebble gives you the primitives and stays out of the way.
//!
//! Start with [`app::App`] and [`ecs::plugin::Plugin`].

// So `#[derive(MaterialParams)]`/`#[derive(ComputeParams)]`'s generated
// `::pebble::...` paths also resolve when the derive is used inside this
// crate's own tests, not just from a downstream consumer.
extern crate self as pebble;

pub mod app;
pub mod assets;
pub mod ecs;
pub mod graphics;
mod macros;
pub mod time;
