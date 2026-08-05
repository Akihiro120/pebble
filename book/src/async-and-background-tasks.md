# Async Systems and Background Tasks

Some work shouldn't block a frame: decoding a large file, a network fetch, a GPU→CPU buffer readback. `BackgroundTasksPlugin::new(worker_count)` registers a small worker pool (`Res<BackgroundTasks>`) for exactly this, with **four** ways to use it depending on what you need back:

| I want... | Use | Result delivery |
|---|---|---|
| A blocking closure run off-thread, native only | `BackgroundTasks::spawn_blocking` | poll the returned `TaskHandle<T>` yourself |
| A future (`async`/`.await`) run off-thread, web-compatible | `BackgroundTasks::spawn_async` | poll the returned `TaskHandle<T>` yourself |
| A whole system that's fire-and-forget async, no result needed | `.detach()` | nothing — genuinely fire-and-forget |
| A future whose result should show up as an ordinary event | `AsyncEventWriter<T>` | automatic — arrives on `EventReader<T>` |

`spawn_blocking` is the odd one out and named for it: there's no OS thread to block in a browser tab, so it's native-only. Everything else in this table works identically on native and web.

## The friendliest option: `AsyncEventWriter<T>`

For the common case — "run this in the background, deliver the result as an event once it's done" — `AsyncEventWriter<T>` combines `spawn_async` with [the event system](./events.md), so consuming the result is completely ordinary:

```rust
struct ReadbackDone(Vec<u8>);

app.add_async_event::<ReadbackDone>();

fn start_readback(events: AsyncEventWriter<ReadbackDone>, buffer: Res<SomeGpuBuffer>) {
    let future = buffer.0.read(); // buffer.0: pebble::wgpu::buffer::Buffer
    events.spawn(async move { ReadbackDone(future.await) });
}

fn on_readback(mut reader: EventReader<ReadbackDone>) {
    for event in reader.iter() {
        // event.0 is the Vec<u8> read back from the GPU
    }
}
```

It sits next to `EventWriter<T>` in the same vocabulary — `EventWriter::send` enqueues an event *now*, `AsyncEventWriter::spawn` enqueues one *once the future resolves*. Register the type with `app.add_async_event::<T>()`, not `add_event` — using the wrong one produces a hint telling you exactly that.

## Polling a task yourself

```rust
let handle: TaskHandle<Vec<u8>> = tasks.spawn_blocking(|| expensive_computation());

// later, once per tick:
match handle.poll() {
    TaskStatus::Pending => {}                          // not done yet, check again next tick
    TaskStatus::Ready(value) => { /* use value */ }
    TaskStatus::Panicked(message) => tracing::error!("task failed: {message}"),
}
```

`spawn_blocking` is native-only (no OS thread in a browser tab); `spawn_async` takes a future and works on both. `TaskStatus::Panicked` — not just a `None`/silent hang — is why `poll()` is preferred over an older-style `try_recv()`.

## Fire-and-forget systems: `.detach()`

A whole system can be async without any of the above, if you genuinely don't need the result back:

```rust
fn save_screenshot(tasks: Res<BackgroundTasks>) -> impl Future<Output = ()> + Send + 'static {
    let tasks = tasks.clone();
    async move {
        // ... write to disk ...
    }
}

app.add_system(SystemStage::Update, save_screenshot.detach());
```

The system runs synchronously as usual — its `SystemParam`s are fetched normally — but instead of doing the work directly, it returns a future, which the scheduler hands to `spawn_async` and moves on from immediately. A real `async fn` can't be used directly here: its returned future borrows every parameter, so it's never `'static` on its own. Extract the owned pieces you need in the ordinary function body, then move only those into the `async move` block you return.

## Fetching a file over HTTP

Same shape as the readback example above — wrap the fetch in a future, spawn it, read the result off an `EventReader` in a later system. Only the body of the future differs between native and web:

```rust
struct FileLoaded(Result<Vec<u8>, String>);

app.add_async_event::<FileLoaded>();

fn start_download(events: AsyncEventWriter<FileLoaded>) {
    events.spawn(async move {
        FileLoaded(fetch_url("https://example.com/data.bin").await)
    });
}

fn on_file_loaded(mut reader: EventReader<FileLoaded>) {
    for FileLoaded(result) in reader.iter() {
        match result {
            Ok(bytes) => { /* ... */ }
            Err(e) => tracing::error!("download failed: {e}"),
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_url(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let window = web_sys::window().unwrap();
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await.map_err(|e| format!("{e:?}"))?.dyn_into().unwrap();
    let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_url(url: &str) -> Result<Vec<u8>, String> {
    reqwest::get(url).await.map_err(|e| e.to_string())?
        .bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}
```

The `#[cfg]` split lives entirely inside `fetch_url` — everything above it (the event, the spawn call, the reader) is identical on both platforms.

## Getting a JS event into the scheduler

Going the other direction — a browser event (a button click, a custom `postMessage`) reaching your systems — doesn't go through `BackgroundTasks` at all, since there's no future to await; the callback fires synchronously whenever the browser decides to call it. The pattern is a plain channel, filled by a `wasm_bindgen` closure registered on the DOM element, drained by an ordinary system into an `EventWriter`:

```rust
#[derive(Clone)]
struct ButtonClicks(crossbeam_channel::Sender<()>, crossbeam_channel::Receiver<()>);

struct ButtonClicked;
app.add_event::<ButtonClicked>();

#[cfg(target_arch = "wasm32")]
fn setup_button_listener(app: &mut App) {
    let (tx, rx) = crossbeam_channel::unbounded();
    app.add_resource(ButtonClicks(tx.clone(), rx));

    let button = web_sys::window().unwrap().document().unwrap()
        .get_element_by_id("my-button").unwrap();
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        let _ = tx.send(());
    }).into_js_value();
    button.add_event_listener_with_callback("click", closure.unchecked_ref()).unwrap();
}

fn drain_button_clicks(clicks: Res<ButtonClicks>, mut writer: EventWriter<ButtonClicked>) {
    while clicks.1.try_recv().is_ok() {
        writer.send(ButtonClicked);
    }
}

fn on_click(mut reader: EventReader<ButtonClicked>) {
    for _ in reader.iter() { /* ... */ }
}
```

This is the same shape `pebble::wgpu::window::WinitWindow` already uses internally for the browser's `resize` event — a `Closure` capturing a `Sender`, registered once at startup, drained by a system every tick. Your gameplay code only ever sees `EventReader<ButtonClicked>`; nothing downstream needs to know the event originated from outside the ECS at all.

## Web support at a glance

| API | Native | Web (wasm32) |
|---|---|---|
| `BackgroundTasks::spawn_blocking` | ✅ | ❌ (queues a job that never runs) |
| `BackgroundTasks::spawn_async` / `.detach()` / `AsyncEventWriter<T>` | ✅ | ✅ |
| `Buffer::read`/`read_as::<T>` | ✅ | ✅ |

The rule of thumb: if it's a **future**, it runs everywhere. If it's a **blocking closure**, it's native-only — there's no thread to block on in a browser tab. [Running on the Web](./running-on-the-web.md) covers the rest of what's platform-specific once graphics enter the picture.
