# Gamepad Input

Native-only — there's no `Gamepads` resource on wasm, and `GamepadPlugin` isn't even compiled for `wasm32-unknown-unknown`.

```rust,ignore
app.add_plugin(GamepadPlugin)
```

If no gamepad backend is available on the current platform, `GamepadPlugin` logs an error and continues without inserting `Gamepads` — treat gamepad support as optional in your own systems if you want graceful degradation.

```rust,ignore
fn read_input(gamepads: Read<Gamepads>) {
    for id in gamepads.ids() {
        if gamepads.button_pressed(id, GamepadButton::South) {
            // jump — this tick only, edge-triggered
        }
        let x = gamepads.axis(id, GamepadAxis::LeftStickX);
    }
}
```

- `button_pressed`/`button_released` are edge-triggered — `true` only the tick the transition happened.
- `button_held` is level-triggered — `true` for as long as it's down.
- `axis` returns a normalized `f32`, `0.0` for a disconnected gamepad.

`GamepadButton`/`GamepadAxis` mirror `gilrs`'s own enums — no `gilrs` type appears in the public API.
