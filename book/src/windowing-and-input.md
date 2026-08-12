# Windowing and Input

`WindowPlugin` opens an OS window via `winit` and inserts two resources: `Window` and `Input`. `GraphicsPlugin` adds it for you; add it directly only if you're assembling your own windowing/backend setup:

```rust,ignore
app.add_plugin(WindowPlugin::new(WindowConfig {
    title: "My Game".into(),
    width: 1280,
    height: 720,
}))
```

`WindowConfig::default()` gives you `"Pebble"` at 1280x720.

`WindowPlugin` also installs the app's runner (via `App::set_runner`) — it hands control to `winit`'s own event loop instead of the default headless polling loop, and works unmodified on both native and `wasm32-unknown-unknown`.

## Input

`Input` gives keyboard/mouse state for the current tick, backed by `winit_input_helper`:

```rust,ignore
fn handle_input(input: Read<Input>) {
    if input.key_pressed(KeyCode::Space) {
        // this tick only
    }
    if input.key_held(KeyCode::KeyW) {
        // every tick it's down
    }
    let (dx, dy) = input.mouse_diff();
    if input.close_requested() {
        // decide yourself whether/how to exit
    }
}
```

- `key_pressed`/`mouse_pressed` are edge-triggered; `key_held`/`mouse_held` are level-triggered.
- `cursor()` — position in window coordinates, `None` if outside the window.
- `cursor_diff()` — cursor movement since last tick, clamped to the window.
- `mouse_diff()` — raw mouse motion since last tick, *not* clamped — useful for a look/orbit camera.
- `scroll_diff()`, `resolution()`, `close_requested()`.

`KeyCode`/`MouseButton` mirror `winit`'s own types — no `winit` type appears in the public API.
