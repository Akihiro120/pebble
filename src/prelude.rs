//! `use pebble::prelude::*;` for the types you need to build an app: `App`,
//! system params, the asset pipeline, and the backend-agnostic rendering
//! traits. Deliberately an explicit, curated list rather than blanket
//! `pub use module::*` re-exports (except where noted) — that way a new
//! item added to an internal module doesn't silently become part of the
//! public prelude surface just by existing.

pub use hecs::Entity;

pub use crate::app::{App, SystemStage};
pub use crate::assets::{
    deps::Dependencies,
    handle::Handle,
    plugin::AssetPlugin,
    singleton_asset::{LazyResource, LazyResourcePlugin},
    storage::{Assets, ProcessedAssets, RawAssetHandle},
    upload::Asset,
};
pub use crate::ecs::{
    events::{AsyncEventWriter, EventReader, EventWriter, Events},
    plugin::Plugin,
    resources::Resources,
    system::{
        AsyncExt, Commands, IntoSystem, Local, OnceExt, Query, Res, ResMut, System, SystemParam,
    },
    system_condition::{
        And, Or, ResourceExists, RunCondition, RunIfExt, RunIfFnExt, SystemSetRunIfExt,
    },
    system_set::IntoSystemSet,
};
pub use crate::rendering::{
    active_frame::ActiveFrame,
    backend::{Backend, ColorTarget, CurrentFrame, DepthTarget, FrameOperations, Pass},
    errors::AcquireError,
    graphics_plugin::GraphicsPlugin,
    render_plugin::{RenderFailure, RenderPlugin},
    sync::{InitReceiver, InitSender, init_channel},
    window::{
        GPUSurfaceHandle, PresentableWindow, WindowConfig, WindowProvider, WindowResource,
        WindowRunner,
    },
    window_plugin::WindowPlugin,
};
pub use crate::audio::{AudioError, AudioOutput, AudioPlugin, PlayingSound, Sound, SoundBuilder};
pub use crate::gamepad::{GamepadAxis, GamepadButton, GamepadId, GamepadPlugin, Gamepads};
pub use crate::threading::{BackgroundTasks, BackgroundTasksPlugin, TaskHandle, TaskStatus};
pub use crate::time::{Time, TimePlugin};
