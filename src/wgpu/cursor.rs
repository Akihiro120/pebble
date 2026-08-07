//! Mirrors `winit::window::CursorIcon` and `winit::window::CursorGrabMode`
//! exactly, so [`Window`](super::window::Window)'s cursor methods never need
//! a raw `winit` type. Convert into winit via `.into()`.

/// The shape of the mouse cursor — mirrors `winit::window::CursorIcon`
/// exactly (same variants, same names).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    #[default]
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
    DndAsk,
    AllResize,
}

impl From<CursorIcon> for winit::window::CursorIcon {
    fn from(value: CursorIcon) -> Self {
        match value {
            CursorIcon::Default => Self::Default,
            CursorIcon::ContextMenu => Self::ContextMenu,
            CursorIcon::Help => Self::Help,
            CursorIcon::Pointer => Self::Pointer,
            CursorIcon::Progress => Self::Progress,
            CursorIcon::Wait => Self::Wait,
            CursorIcon::Cell => Self::Cell,
            CursorIcon::Crosshair => Self::Crosshair,
            CursorIcon::Text => Self::Text,
            CursorIcon::VerticalText => Self::VerticalText,
            CursorIcon::Alias => Self::Alias,
            CursorIcon::Copy => Self::Copy,
            CursorIcon::Move => Self::Move,
            CursorIcon::NoDrop => Self::NoDrop,
            CursorIcon::NotAllowed => Self::NotAllowed,
            CursorIcon::Grab => Self::Grab,
            CursorIcon::Grabbing => Self::Grabbing,
            CursorIcon::EResize => Self::EResize,
            CursorIcon::NResize => Self::NResize,
            CursorIcon::NeResize => Self::NeResize,
            CursorIcon::NwResize => Self::NwResize,
            CursorIcon::SResize => Self::SResize,
            CursorIcon::SeResize => Self::SeResize,
            CursorIcon::SwResize => Self::SwResize,
            CursorIcon::WResize => Self::WResize,
            CursorIcon::EwResize => Self::EwResize,
            CursorIcon::NsResize => Self::NsResize,
            CursorIcon::NeswResize => Self::NeswResize,
            CursorIcon::NwseResize => Self::NwseResize,
            CursorIcon::ColResize => Self::ColResize,
            CursorIcon::RowResize => Self::RowResize,
            CursorIcon::AllScroll => Self::AllScroll,
            CursorIcon::ZoomIn => Self::ZoomIn,
            CursorIcon::ZoomOut => Self::ZoomOut,
            CursorIcon::DndAsk => Self::DndAsk,
            CursorIcon::AllResize => Self::AllResize,
        }
    }
}

/// How the cursor is confined while the window has focus — mirrors
/// `winit::window::CursorGrabMode` exactly.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    /// No grabbing — the cursor can leave the window freely. The default.
    #[default]
    None,
    /// Confined to the window area, but still free to move within it.
    /// Not guaranteed to also hide the cursor — pair with
    /// `Window::set_cursor_visible(false)` if that's the goal.
    Confined,
    /// Locked in place at its current position. Not implemented on every
    /// platform (X11/Windows) — see `Window::set_cursor_grab`'s return value.
    Locked,
}

impl From<CursorGrabMode> for winit::window::CursorGrabMode {
    fn from(value: CursorGrabMode) -> Self {
        match value {
            CursorGrabMode::None => Self::None,
            CursorGrabMode::Confined => Self::Confined,
            CursorGrabMode::Locked => Self::Locked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_icon_conversion_is_positional_not_coincidental() {
        assert_eq!(winit::window::CursorIcon::from(CursorIcon::Default), winit::window::CursorIcon::Default);
        assert_eq!(winit::window::CursorIcon::from(CursorIcon::Pointer), winit::window::CursorIcon::Pointer);
        assert_eq!(winit::window::CursorIcon::from(CursorIcon::NwseResize), winit::window::CursorIcon::NwseResize);
        assert_eq!(winit::window::CursorIcon::from(CursorIcon::AllResize), winit::window::CursorIcon::AllResize);
    }

    #[test]
    fn cursor_grab_mode_conversion_round_trips() {
        assert_eq!(winit::window::CursorGrabMode::from(CursorGrabMode::None), winit::window::CursorGrabMode::None);
        assert_eq!(winit::window::CursorGrabMode::from(CursorGrabMode::Confined), winit::window::CursorGrabMode::Confined);
        assert_eq!(winit::window::CursorGrabMode::from(CursorGrabMode::Locked), winit::window::CursorGrabMode::Locked);
    }
}
