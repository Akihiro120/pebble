//! Gamepad/controller polling — `GamepadPlugin` inserts [`Gamepads`] as a
//! resource and ticks it once per frame in `PreUpdate`, exactly like
//! [`crate::time::TimePlugin`]. `App::new()` already builds this in, so
//! `Res<Gamepads>` works without registering anything yourself.
//!
//! Backend-agnostic, same as [`crate::time`] — nothing here depends on
//! `pebble::wgpu` or any particular rendering backend.

use std::collections::HashSet;

use crate::{
    app::SystemStage,
    ecs::{plugin::Plugin, system::ResMut},
};

/// Mirrors `gilrs::Button`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Unknown,
}

impl From<GamepadButton> for gilrs::Button {
    fn from(value: GamepadButton) -> Self {
        match value {
            GamepadButton::South => Self::South,
            GamepadButton::East => Self::East,
            GamepadButton::North => Self::North,
            GamepadButton::West => Self::West,
            GamepadButton::C => Self::C,
            GamepadButton::Z => Self::Z,
            GamepadButton::LeftTrigger => Self::LeftTrigger,
            GamepadButton::LeftTrigger2 => Self::LeftTrigger2,
            GamepadButton::RightTrigger => Self::RightTrigger,
            GamepadButton::RightTrigger2 => Self::RightTrigger2,
            GamepadButton::Select => Self::Select,
            GamepadButton::Start => Self::Start,
            GamepadButton::Mode => Self::Mode,
            GamepadButton::LeftThumb => Self::LeftThumb,
            GamepadButton::RightThumb => Self::RightThumb,
            GamepadButton::DPadUp => Self::DPadUp,
            GamepadButton::DPadDown => Self::DPadDown,
            GamepadButton::DPadLeft => Self::DPadLeft,
            GamepadButton::DPadRight => Self::DPadRight,
            GamepadButton::Unknown => Self::Unknown,
        }
    }
}

impl From<gilrs::Button> for GamepadButton {
    fn from(value: gilrs::Button) -> Self {
        match value {
            gilrs::Button::South => Self::South,
            gilrs::Button::East => Self::East,
            gilrs::Button::North => Self::North,
            gilrs::Button::West => Self::West,
            gilrs::Button::C => Self::C,
            gilrs::Button::Z => Self::Z,
            gilrs::Button::LeftTrigger => Self::LeftTrigger,
            gilrs::Button::LeftTrigger2 => Self::LeftTrigger2,
            gilrs::Button::RightTrigger => Self::RightTrigger,
            gilrs::Button::RightTrigger2 => Self::RightTrigger2,
            gilrs::Button::Select => Self::Select,
            gilrs::Button::Start => Self::Start,
            gilrs::Button::Mode => Self::Mode,
            gilrs::Button::LeftThumb => Self::LeftThumb,
            gilrs::Button::RightThumb => Self::RightThumb,
            gilrs::Button::DPadUp => Self::DPadUp,
            gilrs::Button::DPadDown => Self::DPadDown,
            gilrs::Button::DPadLeft => Self::DPadLeft,
            gilrs::Button::DPadRight => Self::DPadRight,
            _ => Self::Unknown,
        }
    }
}

/// Mirrors `gilrs::Axis`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
    Unknown,
}

impl From<GamepadAxis> for gilrs::Axis {
    fn from(value: GamepadAxis) -> Self {
        match value {
            GamepadAxis::LeftStickX => Self::LeftStickX,
            GamepadAxis::LeftStickY => Self::LeftStickY,
            GamepadAxis::LeftZ => Self::LeftZ,
            GamepadAxis::RightStickX => Self::RightStickX,
            GamepadAxis::RightStickY => Self::RightStickY,
            GamepadAxis::RightZ => Self::RightZ,
            GamepadAxis::DPadX => Self::DPadX,
            GamepadAxis::DPadY => Self::DPadY,
            GamepadAxis::Unknown => Self::Unknown,
        }
    }
}

/// Identifies one connected gamepad. Opaque, `Copy` — valid for the whole
/// lifetime of the [`Gamepads`] resource that handed it out (via
/// [`Gamepads::ids`]), even across a disconnect/reconnect of a *different*
/// controller.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct GamepadId(gilrs::GamepadId);

/// Every connected gamepad's current state. A self-contained resource —
/// `Res<Gamepads>`/`ResMut<Gamepads>` — inserted by [`GamepadPlugin`].
pub struct Gamepads {
    /// `Mutex`, not a bare `gilrs::Gilrs`: on some platforms (Windows'
    /// WGI backend in particular) `gilrs::Gilrs` holds a
    /// `std::sync::mpsc::Receiver`, which is `Send` but not `Sync` — and
    /// ECS resources must be `Send + Sync` (`hecs::Component`'s bound)
    /// regardless of the fact that this scheduler only ever touches one
    /// resource from one thread at a time, never concurrently.
    gilrs: std::sync::Mutex<gilrs::Gilrs>,
    /// Rebuilt every [`tick`](Self::tick) by draining `gilrs`'s event
    /// queue — `gilrs` itself only exposes a raw event stream plus
    /// continuously-cached "currently held" state, not pre-computed
    /// this-tick edges the way `WinitInputHelper` gives
    /// [`Input`](crate::wgpu::window::Input) for free, so `Gamepads`
    /// computes them by hand instead.
    pressed_this_tick: HashSet<(GamepadId, GamepadButton)>,
    released_this_tick: HashSet<(GamepadId, GamepadButton)>,
}

impl Gamepads {
    fn new() -> Result<Self, Box<gilrs::Error>> {
        Ok(Self {
            gilrs: std::sync::Mutex::new(gilrs::Gilrs::new().map_err(Box::new)?),
            pressed_this_tick: HashSet::new(),
            released_this_tick: HashSet::new(),
        })
    }

    fn tick(&mut self) {
        self.pressed_this_tick.clear();
        self.released_this_tick.clear();
        let mut gilrs = self.gilrs.lock().unwrap();
        while let Some(event) = gilrs.next_event() {
            let id = GamepadId(event.id);
            match event.event {
                gilrs::EventType::ButtonPressed(button, _) => {
                    self.pressed_this_tick.insert((id, button.into()));
                }
                gilrs::EventType::ButtonReleased(button, _) => {
                    self.released_this_tick.insert((id, button.into()));
                }
                _ => {}
            }
        }
    }

    /// Every currently connected gamepad.
    pub fn ids(&self) -> Vec<GamepadId> {
        self.gilrs.lock().unwrap().gamepads().map(|(id, _)| GamepadId(id)).collect()
    }

    pub fn is_connected(&self, id: GamepadId) -> bool {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).is_some()
    }

    /// True for every tick the button remains held. `false` for a
    /// disconnected/unknown `id`.
    pub fn button_held(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).is_some_and(|gamepad| gamepad.is_pressed(button.into()))
    }

    /// True the tick a button goes from "not held" to "held".
    pub fn button_pressed(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.pressed_this_tick.contains(&(id, button))
    }

    /// True the tick a button goes from "held" to "not held".
    pub fn button_released(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.released_this_tick.contains(&(id, button))
    }

    /// Current value of an analog axis, in `-1.0..=1.0`. `0.0` for a
    /// disconnected/unknown `id`.
    pub fn axis(&self, id: GamepadId, axis: GamepadAxis) -> f32 {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).map_or(0.0, |gamepad| gamepad.value(axis.into()))
    }
}

fn tick_gamepads(mut gamepads: ResMut<Gamepads>) {
    gamepads.tick();
}

/// Registers [`Gamepads`] as a resource and advances it once per frame.
///
/// Unlike [`crate::time::TimePlugin`], this is **not** built into
/// `App::new()` — add it yourself, and enable the `gamepad` Cargo feature.
/// If no gamepad backend is available on this platform at all (rare —
/// distinct from "no controller is currently plugged in", which is a
/// completely normal, always-supported state), `build` logs a
/// `tracing::error!` and does not insert [`Gamepads`] — take
/// `Option<Res<Gamepads>>` in systems that need to keep working either way.
///
/// `App::new()` already builds this in, so registering it again yourself
/// (harmless, but unnecessary) does not open a second gamepad backend —
/// idempotent the same way `TimePlugin` is.
pub struct GamepadPlugin;

impl GamepadPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GamepadPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Cheap marker inserted before the (expensive, fallible) real work, so a
/// second `GamepadPlugin::build` call can check "already handled" without
/// opening a second gamepad backend just to discard it.
struct GamepadPluginRan;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        if !app.try_insert_resource(GamepadPluginRan) {
            return;
        }
        match Gamepads::new() {
            Ok(gamepads) => {
                app.add_resource(gamepads);
                app.add_system(SystemStage::PreUpdate, tick_gamepads);
            }
            Err(e) => tracing::error!("GamepadPlugin: failed to initialize gamepad backend: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gamepad_button_round_trips_through_the_gilrs_conversion() {
        let all = [
            GamepadButton::South,
            GamepadButton::East,
            GamepadButton::North,
            GamepadButton::West,
            GamepadButton::C,
            GamepadButton::Z,
            GamepadButton::LeftTrigger,
            GamepadButton::LeftTrigger2,
            GamepadButton::RightTrigger,
            GamepadButton::RightTrigger2,
            GamepadButton::Select,
            GamepadButton::Start,
            GamepadButton::Mode,
            GamepadButton::LeftThumb,
            GamepadButton::RightThumb,
            GamepadButton::DPadUp,
            GamepadButton::DPadDown,
            GamepadButton::DPadLeft,
            GamepadButton::DPadRight,
            GamepadButton::Unknown,
        ];
        for button in all {
            let gilrs_button: gilrs::Button = button.into();
            assert_eq!(GamepadButton::from(gilrs_button), button);
        }
    }
}
