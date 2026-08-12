pub mod flags;
pub mod pipeline_state;

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

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    #[default]
    None,
    Confined,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    SuperLeft,
    SuperRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    Convert,
    KanaMode,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    Hiragana,
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
}

impl From<KeyCode> for winit::keyboard::KeyCode {
    fn from(value: KeyCode) -> Self {
        match value {
            KeyCode::Backquote => Self::Backquote,
            KeyCode::Backslash => Self::Backslash,
            KeyCode::BracketLeft => Self::BracketLeft,
            KeyCode::BracketRight => Self::BracketRight,
            KeyCode::Comma => Self::Comma,
            KeyCode::Digit0 => Self::Digit0,
            KeyCode::Digit1 => Self::Digit1,
            KeyCode::Digit2 => Self::Digit2,
            KeyCode::Digit3 => Self::Digit3,
            KeyCode::Digit4 => Self::Digit4,
            KeyCode::Digit5 => Self::Digit5,
            KeyCode::Digit6 => Self::Digit6,
            KeyCode::Digit7 => Self::Digit7,
            KeyCode::Digit8 => Self::Digit8,
            KeyCode::Digit9 => Self::Digit9,
            KeyCode::Equal => Self::Equal,
            KeyCode::IntlBackslash => Self::IntlBackslash,
            KeyCode::IntlRo => Self::IntlRo,
            KeyCode::IntlYen => Self::IntlYen,
            KeyCode::KeyA => Self::KeyA,
            KeyCode::KeyB => Self::KeyB,
            KeyCode::KeyC => Self::KeyC,
            KeyCode::KeyD => Self::KeyD,
            KeyCode::KeyE => Self::KeyE,
            KeyCode::KeyF => Self::KeyF,
            KeyCode::KeyG => Self::KeyG,
            KeyCode::KeyH => Self::KeyH,
            KeyCode::KeyI => Self::KeyI,
            KeyCode::KeyJ => Self::KeyJ,
            KeyCode::KeyK => Self::KeyK,
            KeyCode::KeyL => Self::KeyL,
            KeyCode::KeyM => Self::KeyM,
            KeyCode::KeyN => Self::KeyN,
            KeyCode::KeyO => Self::KeyO,
            KeyCode::KeyP => Self::KeyP,
            KeyCode::KeyQ => Self::KeyQ,
            KeyCode::KeyR => Self::KeyR,
            KeyCode::KeyS => Self::KeyS,
            KeyCode::KeyT => Self::KeyT,
            KeyCode::KeyU => Self::KeyU,
            KeyCode::KeyV => Self::KeyV,
            KeyCode::KeyW => Self::KeyW,
            KeyCode::KeyX => Self::KeyX,
            KeyCode::KeyY => Self::KeyY,
            KeyCode::KeyZ => Self::KeyZ,
            KeyCode::Minus => Self::Minus,
            KeyCode::Period => Self::Period,
            KeyCode::Quote => Self::Quote,
            KeyCode::Semicolon => Self::Semicolon,
            KeyCode::Slash => Self::Slash,
            KeyCode::AltLeft => Self::AltLeft,
            KeyCode::AltRight => Self::AltRight,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::CapsLock => Self::CapsLock,
            KeyCode::ContextMenu => Self::ContextMenu,
            KeyCode::ControlLeft => Self::ControlLeft,
            KeyCode::ControlRight => Self::ControlRight,
            KeyCode::Enter => Self::Enter,
            KeyCode::SuperLeft => Self::SuperLeft,
            KeyCode::SuperRight => Self::SuperRight,
            KeyCode::ShiftLeft => Self::ShiftLeft,
            KeyCode::ShiftRight => Self::ShiftRight,
            KeyCode::Space => Self::Space,
            KeyCode::Tab => Self::Tab,
            KeyCode::Convert => Self::Convert,
            KeyCode::KanaMode => Self::KanaMode,
            KeyCode::Lang1 => Self::Lang1,
            KeyCode::Lang2 => Self::Lang2,
            KeyCode::Lang3 => Self::Lang3,
            KeyCode::Lang4 => Self::Lang4,
            KeyCode::Lang5 => Self::Lang5,
            KeyCode::NonConvert => Self::NonConvert,
            KeyCode::Delete => Self::Delete,
            KeyCode::End => Self::End,
            KeyCode::Help => Self::Help,
            KeyCode::Home => Self::Home,
            KeyCode::Insert => Self::Insert,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::ArrowDown => Self::ArrowDown,
            KeyCode::ArrowLeft => Self::ArrowLeft,
            KeyCode::ArrowRight => Self::ArrowRight,
            KeyCode::ArrowUp => Self::ArrowUp,
            KeyCode::NumLock => Self::NumLock,
            KeyCode::Numpad0 => Self::Numpad0,
            KeyCode::Numpad1 => Self::Numpad1,
            KeyCode::Numpad2 => Self::Numpad2,
            KeyCode::Numpad3 => Self::Numpad3,
            KeyCode::Numpad4 => Self::Numpad4,
            KeyCode::Numpad5 => Self::Numpad5,
            KeyCode::Numpad6 => Self::Numpad6,
            KeyCode::Numpad7 => Self::Numpad7,
            KeyCode::Numpad8 => Self::Numpad8,
            KeyCode::Numpad9 => Self::Numpad9,
            KeyCode::NumpadAdd => Self::NumpadAdd,
            KeyCode::NumpadBackspace => Self::NumpadBackspace,
            KeyCode::NumpadClear => Self::NumpadClear,
            KeyCode::NumpadClearEntry => Self::NumpadClearEntry,
            KeyCode::NumpadComma => Self::NumpadComma,
            KeyCode::NumpadDecimal => Self::NumpadDecimal,
            KeyCode::NumpadDivide => Self::NumpadDivide,
            KeyCode::NumpadEnter => Self::NumpadEnter,
            KeyCode::NumpadEqual => Self::NumpadEqual,
            KeyCode::NumpadHash => Self::NumpadHash,
            KeyCode::NumpadMemoryAdd => Self::NumpadMemoryAdd,
            KeyCode::NumpadMemoryClear => Self::NumpadMemoryClear,
            KeyCode::NumpadMemoryRecall => Self::NumpadMemoryRecall,
            KeyCode::NumpadMemoryStore => Self::NumpadMemoryStore,
            KeyCode::NumpadMemorySubtract => Self::NumpadMemorySubtract,
            KeyCode::NumpadMultiply => Self::NumpadMultiply,
            KeyCode::NumpadParenLeft => Self::NumpadParenLeft,
            KeyCode::NumpadParenRight => Self::NumpadParenRight,
            KeyCode::NumpadStar => Self::NumpadStar,
            KeyCode::NumpadSubtract => Self::NumpadSubtract,
            KeyCode::Escape => Self::Escape,
            KeyCode::Fn => Self::Fn,
            KeyCode::FnLock => Self::FnLock,
            KeyCode::PrintScreen => Self::PrintScreen,
            KeyCode::ScrollLock => Self::ScrollLock,
            KeyCode::Pause => Self::Pause,
            KeyCode::BrowserBack => Self::BrowserBack,
            KeyCode::BrowserFavorites => Self::BrowserFavorites,
            KeyCode::BrowserForward => Self::BrowserForward,
            KeyCode::BrowserHome => Self::BrowserHome,
            KeyCode::BrowserRefresh => Self::BrowserRefresh,
            KeyCode::BrowserSearch => Self::BrowserSearch,
            KeyCode::BrowserStop => Self::BrowserStop,
            KeyCode::Eject => Self::Eject,
            KeyCode::LaunchApp1 => Self::LaunchApp1,
            KeyCode::LaunchApp2 => Self::LaunchApp2,
            KeyCode::LaunchMail => Self::LaunchMail,
            KeyCode::MediaPlayPause => Self::MediaPlayPause,
            KeyCode::MediaSelect => Self::MediaSelect,
            KeyCode::MediaStop => Self::MediaStop,
            KeyCode::MediaTrackNext => Self::MediaTrackNext,
            KeyCode::MediaTrackPrevious => Self::MediaTrackPrevious,
            KeyCode::Power => Self::Power,
            KeyCode::Sleep => Self::Sleep,
            KeyCode::AudioVolumeDown => Self::AudioVolumeDown,
            KeyCode::AudioVolumeMute => Self::AudioVolumeMute,
            KeyCode::AudioVolumeUp => Self::AudioVolumeUp,
            KeyCode::WakeUp => Self::WakeUp,
            KeyCode::Meta => Self::Meta,
            KeyCode::Hyper => Self::Hyper,
            KeyCode::Turbo => Self::Turbo,
            KeyCode::Abort => Self::Abort,
            KeyCode::Resume => Self::Resume,
            KeyCode::Suspend => Self::Suspend,
            KeyCode::Again => Self::Again,
            KeyCode::Copy => Self::Copy,
            KeyCode::Cut => Self::Cut,
            KeyCode::Find => Self::Find,
            KeyCode::Open => Self::Open,
            KeyCode::Paste => Self::Paste,
            KeyCode::Props => Self::Props,
            KeyCode::Select => Self::Select,
            KeyCode::Undo => Self::Undo,
            KeyCode::Hiragana => Self::Hiragana,
            KeyCode::Katakana => Self::Katakana,
            KeyCode::F1 => Self::F1,
            KeyCode::F2 => Self::F2,
            KeyCode::F3 => Self::F3,
            KeyCode::F4 => Self::F4,
            KeyCode::F5 => Self::F5,
            KeyCode::F6 => Self::F6,
            KeyCode::F7 => Self::F7,
            KeyCode::F8 => Self::F8,
            KeyCode::F9 => Self::F9,
            KeyCode::F10 => Self::F10,
            KeyCode::F11 => Self::F11,
            KeyCode::F12 => Self::F12,
            KeyCode::F13 => Self::F13,
            KeyCode::F14 => Self::F14,
            KeyCode::F15 => Self::F15,
            KeyCode::F16 => Self::F16,
            KeyCode::F17 => Self::F17,
            KeyCode::F18 => Self::F18,
            KeyCode::F19 => Self::F19,
            KeyCode::F20 => Self::F20,
            KeyCode::F21 => Self::F21,
            KeyCode::F22 => Self::F22,
            KeyCode::F23 => Self::F23,
            KeyCode::F24 => Self::F24,
            KeyCode::F25 => Self::F25,
            KeyCode::F26 => Self::F26,
            KeyCode::F27 => Self::F27,
            KeyCode::F28 => Self::F28,
            KeyCode::F29 => Self::F29,
            KeyCode::F30 => Self::F30,
            KeyCode::F31 => Self::F31,
            KeyCode::F32 => Self::F32,
            KeyCode::F33 => Self::F33,
            KeyCode::F34 => Self::F34,
            KeyCode::F35 => Self::F35,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<MouseButton> for winit::event::MouseButton {
    fn from(value: MouseButton) -> Self {
        match value {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
            MouseButton::Back => Self::Back,
            MouseButton::Forward => Self::Forward,
            MouseButton::Other(code) => Self::Other(code),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

impl From<winit::event::TouchPhase> for TouchPhase {
    fn from(value: winit::event::TouchPhase) -> Self {
        match value {
            winit::event::TouchPhase::Started => Self::Started,
            winit::event::TouchPhase::Moved => Self::Moved,
            winit::event::TouchPhase::Ended => Self::Ended,
            winit::event::TouchPhase::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AstcBlock {
    B4x4,
    B5x4,
    B5x5,
    B6x5,
    B6x6,
    B8x5,
    B8x6,
    B8x8,
    B10x5,
    B10x6,
    B10x8,
    B10x10,
    B12x10,
    B12x12,
}

impl From<AstcBlock> for wgpu::AstcBlock {
    fn from(value: AstcBlock) -> Self {
        match value {
            AstcBlock::B4x4 => Self::B4x4,
            AstcBlock::B5x4 => Self::B5x4,
            AstcBlock::B5x5 => Self::B5x5,
            AstcBlock::B6x5 => Self::B6x5,
            AstcBlock::B6x6 => Self::B6x6,
            AstcBlock::B8x5 => Self::B8x5,
            AstcBlock::B8x6 => Self::B8x6,
            AstcBlock::B8x8 => Self::B8x8,
            AstcBlock::B10x5 => Self::B10x5,
            AstcBlock::B10x6 => Self::B10x6,
            AstcBlock::B10x8 => Self::B10x8,
            AstcBlock::B10x10 => Self::B10x10,
            AstcBlock::B12x10 => Self::B12x10,
            AstcBlock::B12x12 => Self::B12x12,
        }
    }
}

impl From<wgpu::AstcBlock> for AstcBlock {
    fn from(value: wgpu::AstcBlock) -> Self {
        match value {
            wgpu::AstcBlock::B4x4 => Self::B4x4,
            wgpu::AstcBlock::B5x4 => Self::B5x4,
            wgpu::AstcBlock::B5x5 => Self::B5x5,
            wgpu::AstcBlock::B6x5 => Self::B6x5,
            wgpu::AstcBlock::B6x6 => Self::B6x6,
            wgpu::AstcBlock::B8x5 => Self::B8x5,
            wgpu::AstcBlock::B8x6 => Self::B8x6,
            wgpu::AstcBlock::B8x8 => Self::B8x8,
            wgpu::AstcBlock::B10x5 => Self::B10x5,
            wgpu::AstcBlock::B10x6 => Self::B10x6,
            wgpu::AstcBlock::B10x8 => Self::B10x8,
            wgpu::AstcBlock::B10x10 => Self::B10x10,
            wgpu::AstcBlock::B12x10 => Self::B12x10,
            wgpu::AstcBlock::B12x12 => Self::B12x12,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AstcChannel {
    Unorm,
    UnormSrgb,
    Hdr,
}

impl From<AstcChannel> for wgpu::AstcChannel {
    fn from(value: AstcChannel) -> Self {
        match value {
            AstcChannel::Unorm => Self::Unorm,
            AstcChannel::UnormSrgb => Self::UnormSrgb,
            AstcChannel::Hdr => Self::Hdr,
        }
    }
}

impl From<wgpu::AstcChannel> for AstcChannel {
    fn from(value: wgpu::AstcChannel) -> Self {
        match value {
            wgpu::AstcChannel::Unorm => Self::Unorm,
            wgpu::AstcChannel::UnormSrgb => Self::UnormSrgb,
            wgpu::AstcChannel::Hdr => Self::Hdr,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    R8Unorm,
    R8Snorm,
    R8Uint,
    R8Sint,
    R16Uint,
    R16Sint,
    R16Unorm,
    R16Snorm,
    R16Float,
    Rg8Unorm,
    Rg8Snorm,
    Rg8Uint,
    Rg8Sint,
    R32Uint,
    R32Sint,
    R32Float,
    Rg16Uint,
    Rg16Sint,
    Rg16Unorm,
    Rg16Snorm,
    Rg16Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Rgba8Snorm,
    Rgba8Uint,
    Rgba8Sint,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgb9e5Ufloat,
    Rgb10a2Uint,
    Rgb10a2Unorm,
    Rg11b10Ufloat,
    R64Uint,
    Rg32Uint,
    Rg32Sint,
    Rg32Float,
    Rgba16Uint,
    Rgba16Sint,
    Rgba16Unorm,
    Rgba16Snorm,
    Rgba16Float,
    Rgba32Uint,
    Rgba32Sint,
    Rgba32Float,
    Stencil8,
    Depth16Unorm,
    Depth24Plus,
    Depth24PlusStencil8,
    Depth32Float,
    Depth32FloatStencil8,
    NV12,
    P010,
    Bc1RgbaUnorm,
    Bc1RgbaUnormSrgb,
    Bc2RgbaUnorm,
    Bc2RgbaUnormSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaUnormSrgb,
    Bc4RUnorm,
    Bc4RSnorm,
    Bc5RgUnorm,
    Bc5RgSnorm,
    Bc6hRgbUfloat,
    Bc6hRgbFloat,
    Bc7RgbaUnorm,
    Bc7RgbaUnormSrgb,
    Etc2Rgb8Unorm,
    Etc2Rgb8UnormSrgb,
    Etc2Rgb8A1Unorm,
    Etc2Rgb8A1UnormSrgb,
    Etc2Rgba8Unorm,
    Etc2Rgba8UnormSrgb,
    EacR11Unorm,
    EacR11Snorm,
    EacRg11Unorm,
    EacRg11Snorm,
    Astc { block: AstcBlock, channel: AstcChannel },
}

impl From<TextureFormat> for wgpu::TextureFormat {
    fn from(format: TextureFormat) -> Self {
        match format {
            TextureFormat::R8Unorm => Self::R8Unorm,
            TextureFormat::R8Snorm => Self::R8Snorm,
            TextureFormat::R8Uint => Self::R8Uint,
            TextureFormat::R8Sint => Self::R8Sint,
            TextureFormat::R16Uint => Self::R16Uint,
            TextureFormat::R16Sint => Self::R16Sint,
            TextureFormat::R16Unorm => Self::R16Unorm,
            TextureFormat::R16Snorm => Self::R16Snorm,
            TextureFormat::R16Float => Self::R16Float,
            TextureFormat::Rg8Unorm => Self::Rg8Unorm,
            TextureFormat::Rg8Snorm => Self::Rg8Snorm,
            TextureFormat::Rg8Uint => Self::Rg8Uint,
            TextureFormat::Rg8Sint => Self::Rg8Sint,
            TextureFormat::R32Uint => Self::R32Uint,
            TextureFormat::R32Sint => Self::R32Sint,
            TextureFormat::R32Float => Self::R32Float,
            TextureFormat::Rg16Uint => Self::Rg16Uint,
            TextureFormat::Rg16Sint => Self::Rg16Sint,
            TextureFormat::Rg16Unorm => Self::Rg16Unorm,
            TextureFormat::Rg16Snorm => Self::Rg16Snorm,
            TextureFormat::Rg16Float => Self::Rg16Float,
            TextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb => Self::Rgba8UnormSrgb,
            TextureFormat::Rgba8Snorm => Self::Rgba8Snorm,
            TextureFormat::Rgba8Uint => Self::Rgba8Uint,
            TextureFormat::Rgba8Sint => Self::Rgba8Sint,
            TextureFormat::Bgra8Unorm => Self::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb => Self::Bgra8UnormSrgb,
            TextureFormat::Rgb9e5Ufloat => Self::Rgb9e5Ufloat,
            TextureFormat::Rgb10a2Uint => Self::Rgb10a2Uint,
            TextureFormat::Rgb10a2Unorm => Self::Rgb10a2Unorm,
            TextureFormat::Rg11b10Ufloat => Self::Rg11b10Ufloat,
            TextureFormat::R64Uint => Self::R64Uint,
            TextureFormat::Rg32Uint => Self::Rg32Uint,
            TextureFormat::Rg32Sint => Self::Rg32Sint,
            TextureFormat::Rg32Float => Self::Rg32Float,
            TextureFormat::Rgba16Uint => Self::Rgba16Uint,
            TextureFormat::Rgba16Sint => Self::Rgba16Sint,
            TextureFormat::Rgba16Unorm => Self::Rgba16Unorm,
            TextureFormat::Rgba16Snorm => Self::Rgba16Snorm,
            TextureFormat::Rgba16Float => Self::Rgba16Float,
            TextureFormat::Rgba32Uint => Self::Rgba32Uint,
            TextureFormat::Rgba32Sint => Self::Rgba32Sint,
            TextureFormat::Rgba32Float => Self::Rgba32Float,
            TextureFormat::Stencil8 => Self::Stencil8,
            TextureFormat::Depth16Unorm => Self::Depth16Unorm,
            TextureFormat::Depth24Plus => Self::Depth24Plus,
            TextureFormat::Depth24PlusStencil8 => Self::Depth24PlusStencil8,
            TextureFormat::Depth32Float => Self::Depth32Float,
            TextureFormat::Depth32FloatStencil8 => Self::Depth32FloatStencil8,
            TextureFormat::NV12 => Self::NV12,
            TextureFormat::P010 => Self::P010,
            TextureFormat::Bc1RgbaUnorm => Self::Bc1RgbaUnorm,
            TextureFormat::Bc1RgbaUnormSrgb => Self::Bc1RgbaUnormSrgb,
            TextureFormat::Bc2RgbaUnorm => Self::Bc2RgbaUnorm,
            TextureFormat::Bc2RgbaUnormSrgb => Self::Bc2RgbaUnormSrgb,
            TextureFormat::Bc3RgbaUnorm => Self::Bc3RgbaUnorm,
            TextureFormat::Bc3RgbaUnormSrgb => Self::Bc3RgbaUnormSrgb,
            TextureFormat::Bc4RUnorm => Self::Bc4RUnorm,
            TextureFormat::Bc4RSnorm => Self::Bc4RSnorm,
            TextureFormat::Bc5RgUnorm => Self::Bc5RgUnorm,
            TextureFormat::Bc5RgSnorm => Self::Bc5RgSnorm,
            TextureFormat::Bc6hRgbUfloat => Self::Bc6hRgbUfloat,
            TextureFormat::Bc6hRgbFloat => Self::Bc6hRgbFloat,
            TextureFormat::Bc7RgbaUnorm => Self::Bc7RgbaUnorm,
            TextureFormat::Bc7RgbaUnormSrgb => Self::Bc7RgbaUnormSrgb,
            TextureFormat::Etc2Rgb8Unorm => Self::Etc2Rgb8Unorm,
            TextureFormat::Etc2Rgb8UnormSrgb => Self::Etc2Rgb8UnormSrgb,
            TextureFormat::Etc2Rgb8A1Unorm => Self::Etc2Rgb8A1Unorm,
            TextureFormat::Etc2Rgb8A1UnormSrgb => Self::Etc2Rgb8A1UnormSrgb,
            TextureFormat::Etc2Rgba8Unorm => Self::Etc2Rgba8Unorm,
            TextureFormat::Etc2Rgba8UnormSrgb => Self::Etc2Rgba8UnormSrgb,
            TextureFormat::EacR11Unorm => Self::EacR11Unorm,
            TextureFormat::EacR11Snorm => Self::EacR11Snorm,
            TextureFormat::EacRg11Unorm => Self::EacRg11Unorm,
            TextureFormat::EacRg11Snorm => Self::EacRg11Snorm,
            TextureFormat::Astc { block, channel } => {
                Self::Astc { block: block.into(), channel: channel.into() }
            }
        }
    }
}

impl From<wgpu::TextureFormat> for TextureFormat {
    fn from(format: wgpu::TextureFormat) -> Self {
        match format {
            wgpu::TextureFormat::R8Unorm => Self::R8Unorm,
            wgpu::TextureFormat::R8Snorm => Self::R8Snorm,
            wgpu::TextureFormat::R8Uint => Self::R8Uint,
            wgpu::TextureFormat::R8Sint => Self::R8Sint,
            wgpu::TextureFormat::R16Uint => Self::R16Uint,
            wgpu::TextureFormat::R16Sint => Self::R16Sint,
            wgpu::TextureFormat::R16Unorm => Self::R16Unorm,
            wgpu::TextureFormat::R16Snorm => Self::R16Snorm,
            wgpu::TextureFormat::R16Float => Self::R16Float,
            wgpu::TextureFormat::Rg8Unorm => Self::Rg8Unorm,
            wgpu::TextureFormat::Rg8Snorm => Self::Rg8Snorm,
            wgpu::TextureFormat::Rg8Uint => Self::Rg8Uint,
            wgpu::TextureFormat::Rg8Sint => Self::Rg8Sint,
            wgpu::TextureFormat::R32Uint => Self::R32Uint,
            wgpu::TextureFormat::R32Sint => Self::R32Sint,
            wgpu::TextureFormat::R32Float => Self::R32Float,
            wgpu::TextureFormat::Rg16Uint => Self::Rg16Uint,
            wgpu::TextureFormat::Rg16Sint => Self::Rg16Sint,
            wgpu::TextureFormat::Rg16Unorm => Self::Rg16Unorm,
            wgpu::TextureFormat::Rg16Snorm => Self::Rg16Snorm,
            wgpu::TextureFormat::Rg16Float => Self::Rg16Float,
            wgpu::TextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb => Self::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba8Snorm => Self::Rgba8Snorm,
            wgpu::TextureFormat::Rgba8Uint => Self::Rgba8Uint,
            wgpu::TextureFormat::Rgba8Sint => Self::Rgba8Sint,
            wgpu::TextureFormat::Bgra8Unorm => Self::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb => Self::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgb9e5Ufloat => Self::Rgb9e5Ufloat,
            wgpu::TextureFormat::Rgb10a2Uint => Self::Rgb10a2Uint,
            wgpu::TextureFormat::Rgb10a2Unorm => Self::Rgb10a2Unorm,
            wgpu::TextureFormat::Rg11b10Ufloat => Self::Rg11b10Ufloat,
            wgpu::TextureFormat::R64Uint => Self::R64Uint,
            wgpu::TextureFormat::Rg32Uint => Self::Rg32Uint,
            wgpu::TextureFormat::Rg32Sint => Self::Rg32Sint,
            wgpu::TextureFormat::Rg32Float => Self::Rg32Float,
            wgpu::TextureFormat::Rgba16Uint => Self::Rgba16Uint,
            wgpu::TextureFormat::Rgba16Sint => Self::Rgba16Sint,
            wgpu::TextureFormat::Rgba16Unorm => Self::Rgba16Unorm,
            wgpu::TextureFormat::Rgba16Snorm => Self::Rgba16Snorm,
            wgpu::TextureFormat::Rgba16Float => Self::Rgba16Float,
            wgpu::TextureFormat::Rgba32Uint => Self::Rgba32Uint,
            wgpu::TextureFormat::Rgba32Sint => Self::Rgba32Sint,
            wgpu::TextureFormat::Rgba32Float => Self::Rgba32Float,
            wgpu::TextureFormat::Stencil8 => Self::Stencil8,
            wgpu::TextureFormat::Depth16Unorm => Self::Depth16Unorm,
            wgpu::TextureFormat::Depth24Plus => Self::Depth24Plus,
            wgpu::TextureFormat::Depth24PlusStencil8 => Self::Depth24PlusStencil8,
            wgpu::TextureFormat::Depth32Float => Self::Depth32Float,
            wgpu::TextureFormat::Depth32FloatStencil8 => Self::Depth32FloatStencil8,
            wgpu::TextureFormat::NV12 => Self::NV12,
            wgpu::TextureFormat::P010 => Self::P010,
            wgpu::TextureFormat::Bc1RgbaUnorm => Self::Bc1RgbaUnorm,
            wgpu::TextureFormat::Bc1RgbaUnormSrgb => Self::Bc1RgbaUnormSrgb,
            wgpu::TextureFormat::Bc2RgbaUnorm => Self::Bc2RgbaUnorm,
            wgpu::TextureFormat::Bc2RgbaUnormSrgb => Self::Bc2RgbaUnormSrgb,
            wgpu::TextureFormat::Bc3RgbaUnorm => Self::Bc3RgbaUnorm,
            wgpu::TextureFormat::Bc3RgbaUnormSrgb => Self::Bc3RgbaUnormSrgb,
            wgpu::TextureFormat::Bc4RUnorm => Self::Bc4RUnorm,
            wgpu::TextureFormat::Bc4RSnorm => Self::Bc4RSnorm,
            wgpu::TextureFormat::Bc5RgUnorm => Self::Bc5RgUnorm,
            wgpu::TextureFormat::Bc5RgSnorm => Self::Bc5RgSnorm,
            wgpu::TextureFormat::Bc6hRgbUfloat => Self::Bc6hRgbUfloat,
            wgpu::TextureFormat::Bc6hRgbFloat => Self::Bc6hRgbFloat,
            wgpu::TextureFormat::Bc7RgbaUnorm => Self::Bc7RgbaUnorm,
            wgpu::TextureFormat::Bc7RgbaUnormSrgb => Self::Bc7RgbaUnormSrgb,
            wgpu::TextureFormat::Etc2Rgb8Unorm => Self::Etc2Rgb8Unorm,
            wgpu::TextureFormat::Etc2Rgb8UnormSrgb => Self::Etc2Rgb8UnormSrgb,
            wgpu::TextureFormat::Etc2Rgb8A1Unorm => Self::Etc2Rgb8A1Unorm,
            wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => Self::Etc2Rgb8A1UnormSrgb,
            wgpu::TextureFormat::Etc2Rgba8Unorm => Self::Etc2Rgba8Unorm,
            wgpu::TextureFormat::Etc2Rgba8UnormSrgb => Self::Etc2Rgba8UnormSrgb,
            wgpu::TextureFormat::EacR11Unorm => Self::EacR11Unorm,
            wgpu::TextureFormat::EacR11Snorm => Self::EacR11Snorm,
            wgpu::TextureFormat::EacRg11Unorm => Self::EacRg11Unorm,
            wgpu::TextureFormat::EacRg11Snorm => Self::EacRg11Snorm,
            wgpu::TextureFormat::Astc { block, channel } => {
                Self::Astc { block: block.into(), channel: channel.into() }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    Uint8,
    Uint8x2,
    Uint8x4,
    Sint8,
    Sint8x2,
    Sint8x4,
    Unorm8,
    Unorm8x2,
    Unorm8x4,
    Snorm8,
    Snorm8x2,
    Snorm8x4,
    Uint16,
    Uint16x2,
    Uint16x4,
    Sint16,
    Sint16x2,
    Sint16x4,
    Unorm16,
    Unorm16x2,
    Unorm16x4,
    Snorm16,
    Snorm16x2,
    Snorm16x4,
    Float16,
    Float16x2,
    Float16x4,
    Float32,
    Float32x2,
    Float32x3,
    Float32x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
    Float64,
    Float64x2,
    Float64x3,
    Float64x4,
    Unorm10_10_10_2,
    Unorm8x4Bgra,
}

impl From<VertexFormat> for wgpu::VertexFormat {
    fn from(format: VertexFormat) -> Self {
        match format {
            VertexFormat::Uint8 => Self::Uint8,
            VertexFormat::Uint8x2 => Self::Uint8x2,
            VertexFormat::Uint8x4 => Self::Uint8x4,
            VertexFormat::Sint8 => Self::Sint8,
            VertexFormat::Sint8x2 => Self::Sint8x2,
            VertexFormat::Sint8x4 => Self::Sint8x4,
            VertexFormat::Unorm8 => Self::Unorm8,
            VertexFormat::Unorm8x2 => Self::Unorm8x2,
            VertexFormat::Unorm8x4 => Self::Unorm8x4,
            VertexFormat::Snorm8 => Self::Snorm8,
            VertexFormat::Snorm8x2 => Self::Snorm8x2,
            VertexFormat::Snorm8x4 => Self::Snorm8x4,
            VertexFormat::Uint16 => Self::Uint16,
            VertexFormat::Uint16x2 => Self::Uint16x2,
            VertexFormat::Uint16x4 => Self::Uint16x4,
            VertexFormat::Sint16 => Self::Sint16,
            VertexFormat::Sint16x2 => Self::Sint16x2,
            VertexFormat::Sint16x4 => Self::Sint16x4,
            VertexFormat::Unorm16 => Self::Unorm16,
            VertexFormat::Unorm16x2 => Self::Unorm16x2,
            VertexFormat::Unorm16x4 => Self::Unorm16x4,
            VertexFormat::Snorm16 => Self::Snorm16,
            VertexFormat::Snorm16x2 => Self::Snorm16x2,
            VertexFormat::Snorm16x4 => Self::Snorm16x4,
            VertexFormat::Float16 => Self::Float16,
            VertexFormat::Float16x2 => Self::Float16x2,
            VertexFormat::Float16x4 => Self::Float16x4,
            VertexFormat::Float32 => Self::Float32,
            VertexFormat::Float32x2 => Self::Float32x2,
            VertexFormat::Float32x3 => Self::Float32x3,
            VertexFormat::Float32x4 => Self::Float32x4,
            VertexFormat::Uint32 => Self::Uint32,
            VertexFormat::Uint32x2 => Self::Uint32x2,
            VertexFormat::Uint32x3 => Self::Uint32x3,
            VertexFormat::Uint32x4 => Self::Uint32x4,
            VertexFormat::Sint32 => Self::Sint32,
            VertexFormat::Sint32x2 => Self::Sint32x2,
            VertexFormat::Sint32x3 => Self::Sint32x3,
            VertexFormat::Sint32x4 => Self::Sint32x4,
            VertexFormat::Float64 => Self::Float64,
            VertexFormat::Float64x2 => Self::Float64x2,
            VertexFormat::Float64x3 => Self::Float64x3,
            VertexFormat::Float64x4 => Self::Float64x4,
            VertexFormat::Unorm10_10_10_2 => Self::Unorm10_10_10_2,
            VertexFormat::Unorm8x4Bgra => Self::Unorm8x4Bgra,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexStepMode {
    Vertex,
    Instance,
}

impl From<VertexStepMode> for wgpu::VertexStepMode {
    fn from(mode: VertexStepMode) -> Self {
        match mode {
            VertexStepMode::Vertex => Self::Vertex,
            VertexStepMode::Instance => Self::Instance,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Front,
    Back,
}

impl From<Face> for wgpu::Face {
    fn from(value: Face) -> Self {
        match value {
            Face::Front => Self::Front,
            Face::Back => Self::Back,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PolygonMode {
    Fill,
    Line,
    Point,
}

impl From<PolygonMode> for wgpu::PolygonMode {
    fn from(value: PolygonMode) -> Self {
        match value {
            PolygonMode::Fill => Self::Fill,
            PolygonMode::Line => Self::Line,
            PolygonMode::Point => Self::Point,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
    Src1,
    OneMinusSrc1,
    Src1Alpha,
    OneMinusSrc1Alpha,
}

impl From<BlendFactor> for wgpu::BlendFactor {
    fn from(value: BlendFactor) -> Self {
        match value {
            BlendFactor::Zero => Self::Zero,
            BlendFactor::One => Self::One,
            BlendFactor::Src => Self::Src,
            BlendFactor::OneMinusSrc => Self::OneMinusSrc,
            BlendFactor::SrcAlpha => Self::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => Self::OneMinusSrcAlpha,
            BlendFactor::Dst => Self::Dst,
            BlendFactor::OneMinusDst => Self::OneMinusDst,
            BlendFactor::DstAlpha => Self::DstAlpha,
            BlendFactor::OneMinusDstAlpha => Self::OneMinusDstAlpha,
            BlendFactor::SrcAlphaSaturated => Self::SrcAlphaSaturated,
            BlendFactor::Constant => Self::Constant,
            BlendFactor::OneMinusConstant => Self::OneMinusConstant,
            BlendFactor::Src1 => Self::Src1,
            BlendFactor::OneMinusSrc1 => Self::OneMinusSrc1,
            BlendFactor::Src1Alpha => Self::Src1Alpha,
            BlendFactor::OneMinusSrc1Alpha => Self::OneMinusSrc1Alpha,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl From<BlendOperation> for wgpu::BlendOperation {
    fn from(value: BlendOperation) -> Self {
        match value {
            BlendOperation::Add => Self::Add,
            BlendOperation::Subtract => Self::Subtract,
            BlendOperation::ReverseSubtract => Self::ReverseSubtract,
            BlendOperation::Min => Self::Min,
            BlendOperation::Max => Self::Max,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl From<CompareFunction> for wgpu::CompareFunction {
    fn from(value: CompareFunction) -> Self {
        match value {
            CompareFunction::Never => Self::Never,
            CompareFunction::Less => Self::Less,
            CompareFunction::Equal => Self::Equal,
            CompareFunction::LessEqual => Self::LessEqual,
            CompareFunction::Greater => Self::Greater,
            CompareFunction::NotEqual => Self::NotEqual,
            CompareFunction::GreaterEqual => Self::GreaterEqual,
            CompareFunction::Always => Self::Always,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
}

impl From<StencilOperation> for wgpu::StencilOperation {
    fn from(value: StencilOperation) -> Self {
        match value {
            StencilOperation::Keep => Self::Keep,
            StencilOperation::Zero => Self::Zero,
            StencilOperation::Replace => Self::Replace,
            StencilOperation::Invert => Self::Invert,
            StencilOperation::IncrementClamp => Self::IncrementClamp,
            StencilOperation::DecrementClamp => Self::DecrementClamp,
            StencilOperation::IncrementWrap => Self::IncrementWrap,
            StencilOperation::DecrementWrap => Self::DecrementWrap,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureSampleType {
    Float { filterable: bool },
    Depth,
    Sint,
    Uint,
}

impl From<TextureSampleType> for wgpu::TextureSampleType {
    fn from(value: TextureSampleType) -> Self {
        match value {
            TextureSampleType::Float { filterable } => Self::Float { filterable },
            TextureSampleType::Depth => Self::Depth,
            TextureSampleType::Sint => Self::Sint,
            TextureSampleType::Uint => Self::Uint,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}

impl From<TextureViewDimension> for wgpu::TextureViewDimension {
    fn from(value: TextureViewDimension) -> Self {
        match value {
            TextureViewDimension::D1 => Self::D1,
            TextureViewDimension::D2 => Self::D2,
            TextureViewDimension::D2Array => Self::D2Array,
            TextureViewDimension::Cube => Self::Cube,
            TextureViewDimension::CubeArray => Self::CubeArray,
            TextureViewDimension::D3 => Self::D3,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StorageTextureAccess {
    WriteOnly,
    ReadOnly,
    ReadWrite,
    Atomic,
}

impl From<StorageTextureAccess> for wgpu::StorageTextureAccess {
    fn from(value: StorageTextureAccess) -> Self {
        match value {
            StorageTextureAccess::WriteOnly => Self::WriteOnly,
            StorageTextureAccess::ReadOnly => Self::ReadOnly,
            StorageTextureAccess::ReadWrite => Self::ReadWrite,
            StorageTextureAccess::Atomic => Self::Atomic,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IndexFormat {
    Uint16,
    Uint32,
}

impl From<IndexFormat> for wgpu::IndexFormat {
    fn from(format: IndexFormat) -> Self {
        match format {
            IndexFormat::Uint16 => Self::Uint16,
            IndexFormat::Uint32 => Self::Uint32,
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

    #[test]
    fn key_code_conversion_is_positional_not_coincidental() {
        assert_eq!(winit::keyboard::KeyCode::from(KeyCode::Backquote), winit::keyboard::KeyCode::Backquote);
        assert_eq!(winit::keyboard::KeyCode::from(KeyCode::KeyW), winit::keyboard::KeyCode::KeyW);
        assert_eq!(winit::keyboard::KeyCode::from(KeyCode::ArrowUp), winit::keyboard::KeyCode::ArrowUp);
        assert_eq!(winit::keyboard::KeyCode::from(KeyCode::ShiftLeft), winit::keyboard::KeyCode::ShiftLeft);
        assert_eq!(winit::keyboard::KeyCode::from(KeyCode::F35), winit::keyboard::KeyCode::F35);
    }

    #[test]
    fn mouse_button_conversion_preserves_the_other_code() {
        assert_eq!(winit::event::MouseButton::from(MouseButton::Left), winit::event::MouseButton::Left);
        assert_eq!(winit::event::MouseButton::from(MouseButton::Other(7)), winit::event::MouseButton::Other(7));
    }

    #[test]
    fn touch_phase_conversion_is_positional() {
        assert_eq!(TouchPhase::from(winit::event::TouchPhase::Started), TouchPhase::Started);
        assert_eq!(TouchPhase::from(winit::event::TouchPhase::Cancelled), TouchPhase::Cancelled);
    }

    #[test]
    fn texture_format_round_trips_including_astc() {
        let astc = TextureFormat::Astc { block: AstcBlock::B4x4, channel: AstcChannel::Hdr };
        let raw = wgpu::TextureFormat::from(astc);
        assert_eq!(TextureFormat::from(raw), astc);
        assert_eq!(wgpu::TextureFormat::from(TextureFormat::Bgra8UnormSrgb), wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    fn index_format_conversion_round_trips() {
        assert_eq!(wgpu::IndexFormat::from(IndexFormat::Uint16), wgpu::IndexFormat::Uint16);
        assert_eq!(wgpu::IndexFormat::from(IndexFormat::Uint32), wgpu::IndexFormat::Uint32);
    }
}
