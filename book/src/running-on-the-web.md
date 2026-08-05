# Running on the Web

Everything in this book runs on `wasm32-unknown-unknown` as well as native, with no code changes to the application logic itself — `pebble::wgpu` already branches internally wherever the two platforms genuinely differ (GPU backend selection, buffer-mapping driven by the browser's microtask queue instead of a worker thread, and so on).

```sh
cargo build --target wasm32-unknown-unknown
```

## The canvas

`pebble::wgpu::window::WinitWindow` looks for a canvas by a fixed element id and renders into it — add one to your HTML:

```html
<canvas id="wgpu_canvas"></canvas>
```

## Bundling

Pulling in `web-sys`/`wasm-bindgen`/`wasm-bindgen-futures` (already `wasm32`-only dependencies of the crate — you don't add them yourself) and bundling with `wasm-bindgen`, `trunk`, or `wasm-pack` is up to your own build setup; Pebble doesn't prescribe one. A minimal `trunk`-based `index.html` needs nothing beyond the canvas above and trunk's usual `<link data-trunk rel="rust" />` tag.

## What actually changes on web

Only one API in this book behaves differently, and [Async Systems and Background Tasks](./async-and-background-tasks.md) already covers exactly why:

| API | Native | Web |
|---|---|---|
| `BackgroundTasks::spawn_blocking` | ✅ | ❌ — queues a job that never runs, there's no OS thread to block |
| `BackgroundTasks::spawn_async` / `.detach()` / `AsyncEventWriter<T>` | ✅ | ✅ — driven by the browser's microtask queue |
| `Buffer::read`/`read_as::<T>` | ✅ | ✅ |

If your project never calls `spawn_blocking` directly, the same binary logic works unmodified on both targets — the render loop, the asset pipeline, materials, textures, camera, compute, all of it. The `#[cfg(target_arch = "wasm32")]` splits you *will* need are the ones you write yourself for genuinely browser-only integration — reading a DOM element, listening for a JS event — exactly the pattern in [Getting a JS event into the scheduler](./async-and-background-tasks.md#getting-a-js-event-into-the-scheduler).

## This book's own deploy pipeline

If you're curious what a real build-and-deploy setup looks like end to end: this book itself is built by `mdbook` and published to GitHub Pages via a GitHub Actions workflow in the same repository (`.github/workflows/deploy-book.yml`) — not a wasm/Pebble deploy specifically, but the same "push to `main`, CI builds it, CI publishes it" shape applies whether the artifact is a book or a `trunk build --release` output directory.
