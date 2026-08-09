//! Demonstrates the three additions from this session: touch (an extension
//! of the existing `Input` resource), gamepad polling (`Gamepads`), and
//! audio playback (`AudioOutput`) — the latter two are built into
//! `App::new()` automatically, same as `Time`, so there's no
//! `.add_plugin(...)` for either below. Touch/gamepad state can't be
//! exercised without real hardware, but this proves the API compiles,
//! wires up, and doesn't panic with nothing connected — and the audio path
//! (press Space) is genuinely testable on any machine with a working
//! output device.

use pebble::prelude::*;
use pebble::wgpu::backend::WGPUPlugin;
use pebble::wgpu::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Input and Audio".to_string(),
            width: 800,
            height: 600,
        }))
        .add_system(SystemStage::Startup, setup)
        .add_system(SystemStage::Update, play_blip_on_space)
        .add_system(SystemStage::Update, log_touches)
        .add_system(SystemStage::Update, log_gamepads)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(mut commands: Commands) -> Option<()> {
    match SoundBuilder::from_file("../assets/audio/blip.wav") {
        Ok(sound) => {
            commands.insert_resource(sound);
            Some(())
        }
        Err(e) => {
            eprintln!("failed to load blip.wav: {e}");
            std::process::exit(1);
        }
    }
}

/// The audio path — genuinely testable on any machine with a working output
/// device, unlike touch/gamepad which need real hardware.
fn play_blip_on_space(input: Res<Input>, blip: Option<Res<Sound>>, audio: Option<Res<AudioOutput>>) {
    if !input.key_pressed(KeyCode::Space) {
        return;
    }
    let (Some(blip), Some(audio)) = (blip, audio) else {
        println!("space pressed, but no audio output device is available");
        return;
    };
    println!("playing blip.wav");
    audio.play(&blip);
}

/// Prints active touch points whenever the count changes — most dev
/// machines have no touchscreen, so `touch_count()` staying at 0 forever is
/// the expected, fully-supported case, not a failure.
fn log_touches(input: Res<Input>, mut last_count: Local<usize>) {
    let count = input.touch_count();
    if count != *last_count {
        println!("touch_count: {count} -> {:?}", input.touches());
        *last_count = count;
    }
}

/// Prints connected gamepads once when the set changes, and any button
/// press. `Option<Res<Gamepads>>` — `GamepadPlugin` only inserts this if a
/// gamepad backend is available at all on this platform; zero gamepads
/// physically plugged in is a completely different, always-supported case
/// (`ids()` just returns empty).
fn log_gamepads(gamepads: Option<Res<Gamepads>>, mut known: Local<Vec<GamepadId>>) {
    let Some(gamepads) = gamepads else { return };
    let ids = gamepads.ids();
    if ids != *known {
        println!("connected gamepads: {}", ids.len());
        *known = ids;
    }

    for &id in known.iter() {
        for button in [GamepadButton::South, GamepadButton::East, GamepadButton::North, GamepadButton::West] {
            if gamepads.button_pressed(id, button) {
                println!("gamepad {id:?}: {button:?} pressed");
            }
        }
    }
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let _pass = active.render_context([0.05, 0.05, 0.08, 1.0]);
    }
}
