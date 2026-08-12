# Audio

Native-only — there's no `AudioOutput` resource on wasm, and `AudioPlugin` isn't compiled for `wasm32-unknown-unknown`.

```rust,ignore
app.add_plugin(AudioPlugin)
```

If no output device is available, `AudioPlugin` logs an error and continues without `AudioOutput` — audio is treated as optional, not fatal to app startup.

## Loading and playing a sound

```rust,ignore
let sound = SoundBuilder::from_file("assets/jump.wav")?;

fn play_it(audio: Read<AudioOutput>, sound: Read<Sound>) {
    audio.play(&sound);          // fire-and-forget
    audio.play_looped(&sound);   // fire-and-forget, infinite loop
    let handle = audio.play_controlled(&sound); // returns a PlayingSound
}
```

`Sound` is decoded, in-memory audio — cheap to clone and play many times. It isn't a pebble asset ([`Handle<T>`](./the-asset-pipeline.md)); store it in a resource or component yourself.

`PlayingSound`, from `play_controlled`, lets you `set_volume`, `pause`, `resume`, `stop`, and check `is_finished()` on one specific in-flight sound.
