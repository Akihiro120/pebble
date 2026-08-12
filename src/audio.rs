use rodio::Source;

use crate::ecs::plugin::Plugin;

/// Errors from loading a sound file or opening the audio device.
#[derive(Debug)]
pub enum AudioError {
    Io(std::io::Error),
    Decode(String),
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

/// Decoded, in-memory audio — cheap to clone and play many times via
/// [`AudioOutput`].
#[derive(Clone)]
pub struct Sound(rodio::buffer::SamplesBuffer);

/// Loads a [`Sound`] from disk.
pub struct SoundBuilder;

impl SoundBuilder {
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

/// A handle to one in-flight sound, from [`AudioOutput::play_controlled`] —
/// pause/resume/stop it, or check whether it's finished.
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

    pub fn is_finished(&self) -> bool {
        self.0.empty()
    }
}

/// The default audio output device — inserted as a resource by [`AudioPlugin`].
/// Native-only; there's no `Audio` resource on wasm.
pub struct AudioOutput {
    sink: rodio::MixerDeviceSink,
}

impl AudioOutput {
    fn new() -> Result<Self, AudioError> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink().map_err(|e| AudioError::Device(e.to_string()))?;
        Ok(Self { sink })
    }

    /// Plays a sound once, fire-and-forget.
    pub fn play(&self, sound: &Sound) {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone());
        player.play();
        player.detach();
    }

    /// Plays a sound on an infinite loop, fire-and-forget.
    pub fn play_looped(&self, sound: &Sound) {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone().repeat_infinite());
        player.play();
        player.detach();
    }

    /// Plays a sound and returns a [`PlayingSound`] handle to control it —
    /// pause, stop, adjust volume, or check if it's finished.
    pub fn play_controlled(&self, sound: &Sound) -> PlayingSound {
        let player = rodio::Player::connect_new(self.sink.mixer());
        player.append(sound.0.clone());
        player.play();
        PlayingSound(player)
    }
}

/// Opens the default audio output device and inserts [`AudioOutput`]. If no
/// device is available, logs an error and continues without one — audio is
/// treated as optional, not fatal to app startup.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        match AudioOutput::new() {
            Ok(output) => app.insert_resource(output),
            Err(e) => {
                tracing::error!("AudioPlugin: failed to open the default audio output device: {e}");
                app
            }
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
        assert!(sound.0.total_duration().is_some_and(|d| !d.is_zero()));
    }
}
