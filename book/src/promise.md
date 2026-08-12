# Promise

`Promise<T>` is a one-off async result you poll each tick — not a resource, not registered anywhere, just a plain value you store wherever fits (a `Local<T>`, a field on your own resource or component). Pebble uses it internally for GPU backend acquisition and for [`Buffer::read`](./buffers.md#reading-a-buffer-back).

Create a paired producer/consumer:

```rust,ignore
let (fulfiller, promise) = Promise::new();
```

Whoever produces the value calls `fulfiller.fulfill(value)` — often from a background task or a GPU callback. Whoever needs the result polls the `Promise` each tick:

```rust,ignore
fn check_result(mut promise: Local<Option<Promise<u32>>>) {
    let Some(p) = promise.as_ref() else { return };
    match p.poll() {
        PromiseState::Ready(value) => {
            println!("got {value}");
            *promise = None;
        }
        PromiseState::Pending => {}
        PromiseState::Disconnected => {
            // the Fulfiller was dropped without ever fulfilling
            *promise = None;
        }
    }
}
```

`poll()` is non-blocking and safe to call every tick. `PromiseState::Ready(T)` is only ever returned once — polling again after that returns `Disconnected`, since the value has already been taken.
