# Gamepad Input

`Gamepads` (`pebble::gamepad`, re-exported from `pebble::prelude`) is every connected controller's current state. `App::new()` inserts it automatically — same as [`Time`](./time.md) — so any system can fetch it directly, no plugin registration needed:

```rust
use pebble::prelude::*;

fn drive(gamepads: Res<Gamepads>) {
    for id in gamepads.ids() {
        if gamepads.button_pressed(id, GamepadButton::South) {
            // fires once, the tick the button goes down
        }
        let steer = gamepads.axis(id, GamepadAxis::LeftStickX);
        // -1.0..=1.0
    }
}
```

`GamepadButton`/`GamepadAxis` are Pebble's own types (mirroring `gilrs`'s exactly) — no raw `gilrs` type crosses `Gamepads`'s API. `GamepadId` identifies one connected controller; it's `Copy`, valid for the whole lifetime of the `Gamepads` resource that handed it out.

## What's tracked

State refreshes once per tick, in `PreUpdate`, before other systems run:

- **Buttons**: `button_pressed`/`button_released` (edge-triggered — true only the tick the state changes) and `button_held` (true for as long as it's down) — same shape as [`Input`](./input.md)'s keyboard/mouse accessors.
- **Axes**: `axis(id, axis)` — current analog value, `-1.0..=1.0`.
- **Connection**: `ids()` (every currently connected gamepad) and `is_connected(id)`.

`button_held`/`axis` return `false`/`0.0` for a disconnected or unrecognized `id` rather than panicking — a controller unplugging mid-game is a normal event, not an error condition to guard against everywhere.

## Zero gamepads connected is the normal case

`Gamepads` tracks whichever controllers happen to be plugged in *right now* — most players have none connected most of the time, and `ids()` returning an empty `Vec` is exactly what that looks like, not a failure state. Don't special-case it beyond however your own input-handling code naturally does (skip iterating an empty `ids()` list).

## If no gamepad backend is available at all

Distinct from "no controller plugged in": on a platform with no working gamepad backend at all (rare), `Gamepads` won't be inserted, and `App::new()` logs a `tracing::error!` explaining why. Take `Option<Res<Gamepads>>` instead of `Res<Gamepads>` in systems that need to keep working either way.

## Platform notes

Backed by [`gilrs`](https://docs.rs/gilrs) — supported on Windows, Linux (requires `libudev-dev` to compile), and macOS. No `wasm32` target support currently.
