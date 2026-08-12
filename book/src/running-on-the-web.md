# Running on the Web

Pebble builds for `wasm32-unknown-unknown` — `WindowPlugin`'s runner and the whole rendering pipeline are written to work unmodified on both native and web. A few things differ:

- **No `Audio`/`Gamepad`.** `AudioPlugin`/`GamepadPlugin` (and the `Sound`/`Gamepads` types they depend on) aren't compiled at all on wasm — don't reference them behind anything other than `#[cfg(not(target_arch = "wasm32"))]`.
- **The canvas is inserted automatically.** `winit` doesn't do this on its own; `WindowPlugin` asks it to (`with_append(true)`) so a window actually shows up in the page without hand-rolled DOM code.
- **The event loop is non-blocking.** Natively, `winit`'s `run` blocks forever; on wasm it uses `EventLoopExtWebSys::spawn`, which registers the loop with the browser and returns immediately. This is handled inside `WindowPlugin` — you don't need to branch on it yourself.
- **The default headless loop busy-polls instead of sleeping.** There's no real OS thread to sleep on wasm, so if you never register a windowing plugin (uncommon), the fallback loop just spins. In practice this path is only reached before a windowing plugin's runner takes over.
- **No `ADDRESS_MODE_CLAMP_TO_BORDER`.** The GPU backend requests this feature only on native; `SamplerKind::NearestClampBorder` falls back to clamp-to-edge on wasm accordingly.

Building:

```sh
cargo build --target wasm32-unknown-unknown
```

You'll still need your own bundling/serving setup (`wasm-bindgen`, `trunk`, or similar) — pebble doesn't ship one.
