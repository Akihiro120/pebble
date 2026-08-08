# Audio

`AudioOutput` (`pebble::audio`, re-exported from `pebble::prelude`) is the audio output device. `App::new()` opens it and inserts it automatically — same as [`Time`](./time.md)/[`Gamepads`](./gamepad-input.md) — so any system can fetch it directly:

```rust
use pebble::prelude::*;

fn setup(mut commands: Commands) -> Option<()> {
    let sound = SoundBuilder::from_file("assets/blip.wav").ok()?;
    commands.insert_resource(sound);
    Some(())
}

fn play_it(input: Res<Input>, sound: Res<Sound>, audio: Res<AudioOutput>) {
    if input.key_pressed(KeyCode::Space) {
        audio.play(&sound);
    }
}
```

## Loading a sound

[`SoundBuilder::from_file`](../src/audio.rs) decodes a whole audio file up front (wav, mp3, flac, vorbis — whatever `rodio`'s default decoder supports) into a [`Sound`](../src/audio.rs) — cheap to clone and play more than once or simultaneously. This is a one-shot, synchronous call, not part of any retry pipeline (same reasoning as [`load_gltf`](./loading-gltf-models.md#why-result-not-optionsilent-retry)): a missing file or corrupt/unsupported format is a real `Err` you handle immediately, not a silent forever-retry. Fine for sound effects and short music clips; a very long track means a correspondingly large in-memory buffer, since nothing here streams from disk.

## Playing a sound

- **`audio.play(&sound)`** / **`audio.play_looped(&sound)`** — fire-and-forget. Plays out fully (or loops forever) regardless of whether you keep anything around afterward.
- **`audio.play_controlled(&sound) -> PlayingSound`** — same, but returns a handle for `set_volume`/`pause`/`resume`/`stop`/`is_finished`. **Dropping the handle stops playback** — keep it alive (a component field, a resource) for as long as you want the sound to keep playing; that's why `play`/`play_looped` exist as separate methods that hand back nothing to accidentally drop.

## If no output device is available at all

`App::new()` logs a `tracing::error!` and leaves `AudioOutput` uninserted rather than panicking. Take `Option<Res<AudioOutput>>` instead of `Res<AudioOutput>` in systems that need to keep working either way (a CI runner or a headless machine with no audio hardware, say).

## Platform notes

Backed by [`rodio`](https://docs.rs/rodio)/`cpal` — supported on Windows (WASAPI), Linux (ALSA, requires dev headers to compile), and macOS (CoreAudio). **No `wasm32` support at all** — `rodio` doesn't build on `wasm32-unknown-unknown`. This is a known, permanent-for-now gap, not a missing feature flag; a web build needs a different audio approach entirely (not covered by this module).
