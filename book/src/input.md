# Input

`pebble::wgpu::window::Input` (re-exported from `pebble::wgpu::prelude`) is the frame's keyboard/mouse/window input state. `WGPUPlugin` inserts it automatically, so any system can fetch it directly:

```rust
use pebble::prelude::*;
use pebble::wgpu::prelude::*;

fn movement(input: Res<Input>, mut query: Query<&mut Transform>) {
    for transform in query.iter() {
        if input.key_held(KeyCode::KeyW) {
            transform.position.z += 1.0;
        }
        if input.key_pressed(KeyCode::Space) {
            // fires once, the step the key goes down
        }
        if input.mouse_held(MouseButton::Left) {
            let (dx, dy) = input.mouse_diff();
            // e.g. rotate a camera by (dx, dy)
        }
    }
}
```

`Input` is a plain resource — `Res<Input>`, no different from any other `Res<T>`. There's no concrete backend type to name and no separate lookup step: unlike `WindowResource<W>` (which needs `W`, e.g. `WindowResource<WinitWindow>`, because it also carries the raw surface handle), `Input` doesn't depend on which backend produced it, so `WindowPlugin` inserts it as its own top-level resource in addition to nesting it inside `WindowResource<W>::exposed`.

`KeyCode` and `MouseButton` are Pebble's own types (mirroring `winit`'s exactly) — like every other value type in `pebble::wgpu`, no raw `winit` type crosses `Input`'s API, so nothing here requires depending on `winit` yourself.

## Cheap to clone, no guard to hold

Every accessor (`key_pressed`, `key_held`, `mouse_diff`, ...) locks internally and returns a plain value — there's no `MutexGuard` or similar to keep alive across a match arm or loop body. Clone `Input` freely; it's an `Arc` internally, so every clone reads the same underlying state.

## What's tracked

State refreshes once per step, before systems run, so every accessor below reflects that step:

- **Keyboard**: `key_pressed`/`key_released` (edge-triggered — true only the step the state changes) and `key_held` (true for as long as it's down), plus `held_shift`/`held_control`/`held_alt` for the common modifier check. All physical-key based (`KeyCode`), so bindings stay put across keyboard layouts — the right choice for game controls.
- **Mouse**: `mouse_pressed`/`mouse_released`/`mouse_held` (same edge/level distinction, `MouseButton`), `cursor()` (position in pixels, `None` when unfocused), `cursor_diff()` (frame-to-frame position delta), `mouse_diff()` (raw device motion — the one to use for a captured-mouse camera, since it isn't clamped to the window like `cursor_diff`), and `scroll_diff()`.
- **Window**: `close_requested()`, `resolution()`, `dropped_file()`, `delta_time()`.

Text entry (IME-aware logical keys, typed characters) isn't covered — `Input` is scoped to physical input suitable for game controls and simple UI toggles, not a text field implementation.

`Input::delta_time()` is a side effect of how `winit_input_helper` times its own step cycle, not a general-purpose clock — it's `Option<Duration>` (`None` on the first step), and only exists once a window backend has inserted `Input` at all. For frame delta/elapsed time/fps in any app, graphical or not, reach for [`Time`](./time.md) instead.
