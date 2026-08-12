use std::collections::HashSet;

use crate::ecs::{plugin::Plugin, resources::Write, system::SystemStage};

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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct GamepadId(gilrs::GamepadId);

pub struct Gamepads {
    // Mutex, not a bare gilrs::Gilrs: on some platforms (Windows' WGI
    // backend) gilrs::Gilrs holds a std::sync::mpsc::Receiver, which is
    // Send but not Sync — and resources must be Send + Sync regardless of
    // this scheduler only ever touching one resource from one thread at a
    // time, never concurrently.
    gilrs: std::sync::Mutex<gilrs::Gilrs>,
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

    pub fn ids(&self) -> Vec<GamepadId> {
        self.gilrs.lock().unwrap().gamepads().map(|(id, _)| GamepadId(id)).collect()
    }

    pub fn is_connected(&self, id: GamepadId) -> bool {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).is_some()
    }

    pub fn button_held(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).is_some_and(|gamepad| gamepad.is_pressed(button.into()))
    }

    pub fn button_pressed(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.pressed_this_tick.contains(&(id, button))
    }

    pub fn button_released(&self, id: GamepadId, button: GamepadButton) -> bool {
        self.released_this_tick.contains(&(id, button))
    }

    pub fn axis(&self, id: GamepadId, axis: GamepadAxis) -> f32 {
        self.gilrs.lock().unwrap().connected_gamepad(id.0).map_or(0.0, |gamepad| gamepad.value(axis.into()))
    }
}

fn tick_gamepads(mut gamepads: Write<Gamepads>) {
    gamepads.tick();
}

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        match Gamepads::new() {
            Ok(gamepads) => app.insert_resource(gamepads).add_system(SystemStage::PreUpdate, tick_gamepads),
            Err(e) => {
                tracing::error!("GamepadPlugin: failed to initialize gamepad backend: {e}");
                app
            }
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
