# Window Control

`pebble::wgpu::window::Window` (re-exported from `pebble::wgpu::prelude`) is runtime control over the OS window — cursor, title, size, fullscreen. `WGPUPlugin` inserts it automatically, right alongside `Input`:

```rust
use pebble::prelude::*;
use pebble::wgpu::prelude::*;

fn captured_mouse_camera(input: Res<Input>, window: Res<Window>, mut hidden: Local<bool>) {
    if input.key_pressed(KeyCode::KeyC) {
        *hidden = !*hidden;
        window.set_cursor_visible(!*hidden);
        window.set_cursor_grab(if *hidden { CursorGrabMode::Locked } else { CursorGrabMode::None });
    }
}
```

`Window` is a plain resource — `Res<Window>`, no different from `Res<Input>`. It's *not* `WindowResource<WinitWindow>::handle`: that's `Arc<winit::window::Window>`, every raw `winit` method included, and requires naming the concrete backend type to reach at all. `Window` needs neither — cheap to clone (an `Arc` internally), every method forwards straight to the OS window.

## What it covers

- **Cursor**: `set_cursor_icon` (`CursorIcon` — Pebble's own type, mirroring `winit`'s exactly), `set_cursor_visible`, `set_cursor_grab` (`CursorGrabMode::None`/`Confined`/`Locked`), `set_cursor_position`. The grab/position setters return `bool` rather than panicking, since whether a platform supports a given mode is a runtime fact about that platform, not a bug — see `CursorGrabMode`'s variant docs for which platforms support which.
- **Size**: `inner_size` (physical pixels), `set_inner_size` (a request — the OS may not grant it exactly, or at all for a maximized/tiled window), `set_min_inner_size`/`set_max_inner_size`, `set_resizable`.
- **Window state**: `set_title`, `set_visible`, `set_minimized`, `set_maximized`, `set_decorations`, `focus`, `set_fullscreen`/`is_fullscreen` (borderless, on the window's current monitor — not exclusive fullscreen or multi-monitor selection, the overwhelmingly common case for a game without pulling in `winit`'s monitor-enumeration types).

## Scope

This is deliberately winit-shaped, not a generic `WindowProvider` capability — a hand-rolled window backend (SDL2, a headless context) has no obligation to provide anything like it. That's also why it isn't inserted by the generic `WindowPlugin<W>`: `WGPUPlugin` adds a small dedicated `WindowControlPlugin` right after `WindowPlugin<WinitWindow>` specifically. If you're composing `WindowPlugin<WinitWindow>` yourself without going through `WGPUPlugin` (see [Windows and Backends](./windows-and-backends.md#owning-the-graphics-backend-yourself)), add `WindowControlPlugin` the same way to get `Res<Window>`.
