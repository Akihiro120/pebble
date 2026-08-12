# Window Control

`Window`, inserted by `WindowPlugin`, gives runtime control over the OS window — no raw `winit` type appears in its public API:

```rust,ignore
fn adjust_window(window: Read<Window>) {
    window.set_title("Paused");
    window.set_inner_size(1920, 1080);
    window.set_fullscreen(true);
    window.set_cursor_visible(false);
    window.set_cursor_grab(CursorGrabMode::Locked);
}
```

Other methods: `inner_size()`, `set_resizable`, `set_visible`, `set_minimized`, `set_maximized`, `set_decorations`, `focus`, `is_fullscreen`, `set_cursor_icon`, `request_redraw`.

`set_fullscreen(true)` uses borderless fullscreen. `set_cursor_grab` takes a `CursorGrabMode` (`None`/`Confined`/`Locked`) and returns whether the platform honored it.
