//! Audio playback — `AudioPlugin` inserts [`AudioOutput`] as a resource,
//! opening the default output device once at startup, exactly like
//! [`crate::time::TimePlugin`]. `App::new()` already builds this in, so
//! `Res<AudioOutput>` works without registering anything yourself.
//!
//! **No wasm32 support** — the underlying `rodio`/`cpal` stack doesn't
//! build on `wasm32-unknown-unknown` at all. A known gap, not solved here.
//!
//! Backend-agnostic, same as [`crate::time`]/[`crate::gamepad`] — nothing
//! here depends on `pebble::wgpu` or any particular rendering backend.
//! `Sound` needs no [`Asset`](crate::assets::upload::Asset)/`Handle`/GPU
//! pipeline — it's not a GPU resource, and decoding it is a synchronous,
//! one-shot call, not something worth an async pipeline (same reasoning as
//! `wgpu::gltf_loader::load_gltf`).

use rodio::Source;

use crate::ecs::plugin::Plugin;

#[derive(Debug)]
pub enum AudioError {
    Io(std::io::Error),
    /// Wraps `rodio::decoder::DecoderError`'s `Display` output — `rodio`
    /// stays an implementation detail, not exposed in this crate's own
    /// error type.
    Decode(String),
    /// Wraps `rodio::stream::DeviceSinkError`'s `Display` output — e.g. no
    /// output device found.
    Device(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read sound file: {e}"),
            Self::Decode(msg) => write!(f, "failed to decode audio: {msg}"),
            Self::Device(msg) => write!(f, "audio output device error: {msg}"),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Decoded sound data, ready to play — cheap to clone (an `Arc`-backed
/// sample buffer internally) and to play more than once or simultaneously.
/// Plain data — the only way to construct one is [`SoundBuilder`].
#[derive(Clone)]
pub struct Sound(rodio::buffer::SamplesBuffer);

/// Builds a [`Sound`] by decoding an audio file. Fully decodes up front
/// (not a streaming decoder) — fine for sound effects and short music
/// clips; a very long track means a correspondingly large in-memory buffer.
pub struct SoundBuilder;

impl SoundBuilder {
    /// Supports whatever formats `rodio`'s default decoder does (wav, mp3,
    /// flac, vorbis, ...). Reads and decodes the whole file synchronously —
    /// this is a one-shot call, not part of any retry pipeline, so a
    /// missing file or unsupported/corrupt format is a real `Err` you
    /// handle immediately, not a silent forever-retry.
    pub fn from_file(path: &str) -> Result<Sound, AudioError> {
        let file = std::fs::File::open(path)?;
        let decoder = rodio::Decoder::new(std::io::BufReader::new(file))
            .map_err(|e| AudioError::Decode(e.to_string()))?;
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let samples: Vec<f32> = decoder.collect();
        Ok(Sound(rodio::buffer::SamplesBuffer::new(channels, sample_rate, samples)))
    }
}

/// A sound actively playing, with volume/pause/stop control — returned by
/// [`AudioOutput::play_controlled`]. **Dropping this stops playback** (the
/// same RAII behavior as closing an audio stream elsewhere) — for
/// "play it and don't worry about the handle," use
/// [`AudioOutput::play`]/[`AudioOutput::play_looped`] instead, which don't
/// hand back anything to accidentally drop.
pub struct PlayingSound(rodio::Player);

impl PlayingSound {
    pub fn set_volume(&self, volume: f32) {
        self.0.set_volume(volume);
    }

    pub fn pause(&self) {
        self.0.pause();
    }

    pub fn resume(&self) {
        self.0.play();
    }

    pub fn stop(&self) {
        self.0.stop();
    }

    /// True once playback has finished (or after [`stop`](Self::stop)).
    pub fn is_finished(&self) -> bool {
        self.0.empty()
    }
}

/// The audio output device — a resource, inserted by [`AudioPlugin`].
pub struct AudioOutput {
    sink: rodio::MixerDeviceSink,
}

impl AudioOutput {
    fn new() -> Result<Self, AudioError> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| AudioError::Device(e.to_string()))?;
        Ok(Self { sink })
    }

    /// Fire-and-forget playback — plays out fully regardless of whether you
    /// keep anything around afterward. For volume/pause/stop control, use
    /// [`play_controlled`](Self::play_controlled) instead.
    pub fn play(&self, sound: &Sound) {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone());
        player.play();
        player.detach();
    }

    /// Same as [`play`](Self::play), looped forever — stop it early via
    /// [`play_controlled`](Self::play_controlled) instead if you need to be
    /// able to turn it off.
    pub fn play_looped(&self, sound: &Sound) {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone().repeat_infinite());
        player.play();
        player.detach();
    }

    /// Same as [`play`](Self::play), but returns a [`PlayingSound`] handle
    /// for volume/pause/stop control. Remember: dropping the handle stops
    /// playback — keep it alive (e.g. as a component/resource field) for as
    /// long as you want the sound to keep playing.
    pub fn play_controlled(&self, sound: &Sound) -> PlayingSound {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone());
        player.play();
        PlayingSound(player)
    }
}

/// Registers [`AudioOutput`] as a resource, opening the default output
/// device once at startup.
///
/// If no output device is available at all, `build` logs a
/// `tracing::error!` and does not insert [`AudioOutput`] — take
/// `Option<Res<AudioOutput>>` in systems that need to keep working either
/// way.
///
/// `App::new()` already builds this in, so registering it again yourself
/// (harmless, but unnecessary) does not open a second output stream —
/// idempotent the same way `TimePlugin` is.
pub struct AudioPlugin;

impl AudioPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AudioPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Cheap marker inserted before the (expensive, fallible) real work, so a
/// second `AudioPlugin::build` call can check "already handled" without
/// opening a second output stream just to discard it.
struct AudioPluginRan;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut crate::prelude::App) {
        if !app.try_insert_resource(AudioPluginRan) {
            return;
        }
        match AudioOutput::new() {
            Ok(output) => {
                app.add_resource(output);
            }
            Err(e) => tracing::error!("AudioPlugin: failed to open the default audio output device: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_file_on_a_missing_path_returns_a_real_err() {
        let result = SoundBuilder::from_file("does/not/exist.wav");
        assert!(result.is_err());
    }

    #[test]
    fn from_file_decodes_a_valid_wav_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/audio/tone.wav");
        let sound = SoundBuilder::from_file(path).expect("fixture should decode cleanly");
        // Just confirms decoding actually produced audio, without needing a
        // real output device (Sound holds no device handle at all).
        assert!(sound.0.total_duration().is_some_and(|d| !d.is_zero()));
    }
}
