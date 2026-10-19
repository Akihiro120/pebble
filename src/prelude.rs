//! The things almost every pebble app names, in one import.
//!
//! ```ignore
//! use pebble::prelude::*;
//! ```
//!
//! This is a convenience, not a second API: every item here is re-exported
//! from its real module, and that path stays the canonical one. Reach past
//! the prelude for anything more specialised — the graphics builders, the
//! `wgpu` type wrappers, the window controls — which are deliberately not
//! here, because a glob import that pulls in two hundred names stops being
//! an aid to reading.

pub use hecs::Entity;

pub use crate::app::{App, AppExit, AppHandle, BackendReady};
pub use crate::assets::handle::Handle;
pub use crate::ecs::{
    commands::Commands,
    condition::{ConditionExt, not, resource_exists},
    events::{EventReader, EventWriter, Events},
    local::Local,
    observers::Trigger,
    plugin::Plugin,
    promise::{Promise, PromiseState},
    query::Query,
    resources::{Read, Resources, Write},
    stream::{Emitter, Stream, StreamState},
    system::{ENGINE_READY_PRIORITY, SystemStage},
    system_param::{Chain, IntoSystemConfig, SystemParam},
};
pub use crate::time::{Time, TimePlugin};

// Windowing and the GPU are behind the same `winit`/`wgpu` stack everywhere
// pebble builds, so these need no cfg — but they are the only graphics items
// in the prelude on purpose. `GraphicsPlugin` and `WindowConfig` are what an
// app names before it can draw anything at all; everything else in
// `graphics::` is reached by its own path.
pub use crate::graphics::{
    GraphicsPlugin,
    window::{Input, Redraw, RedrawPolicy, Window, WindowConfig, WindowPlugin},
};
