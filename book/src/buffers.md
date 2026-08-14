# Buffers

`Buffer` is an opaque GPU buffer — no raw `wgpu` type appears in its public API. Build one with `BufferBuilder`:

```rust,ignore
let vertex_buffer = BufferBuilder::with_data(bytemuck::cast_slice(&vertices))
    .with_label("Vertex Buffer")
    .with_usage(BufferUsages::VERTEX)
    .build(&backend);

let empty = BufferBuilder::empty(1024)
    .with_usage(BufferUsages::STORAGE | BufferUsages::COPY_DST)
    .build(&backend);
```

`.with_uniform()`/`.with_storage()` are shortcuts for the common `UNIFORM | COPY_DST` and `STORAGE | COPY_DST` usage combos.

- `write(data)` — overwrites the buffer's contents from the start.
- `write_at(offset, data)` — overwrites starting at a byte offset.
- `size()` — the buffer's size in bytes.

## Reading a buffer back

`read()` copies the buffer's contents back to the CPU, returning a [`Promise<Vec<u8>>`](./promise.md) — poll it each tick until it resolves. Requires `BufferUsages::COPY_SRC`:

```rust,ignore
fn kick_off(buffer: Read<SomeBuffer>, mut promise: Local<Option<Promise<Vec<u8>>>>) {
    *promise = Some(buffer.0.read());
}

fn check(mut promise: Local<Option<Promise<Vec<u8>>>>) {
    if let Some(p) = promise.as_ref() {
        if let PromiseState::Ready(bytes) = p.poll() {
            // ...
            *promise = None;
        }
    }
}
```

## Dynamic buffers

`DynamicBuffer` holds many fixed-size elements, each individually writable and bindable at an aligned offset — for per-object uniform data, for example. Build one with `DynamicBufferBuilder`; the per-element stride is rounded up to the device's required offset alignment for you:

```rust,ignore
let per_object = DynamicBufferBuilder::uniform(std::mem::size_of::<ObjectData>() as u64, 100)
    .build(&backend);

per_object.write_element(object_index, bytemuck::bytes_of(&data));
```

Bind a specific element with `BindGroupBuilder::with_dynamic_buffer`, then pass its byte offset (`index * stride()`) in the `offsets` slice when you `set_bind_group` during a render/compute pass — see [Bind Groups and Layouts](./bind-groups.md) and [Recording a Render Pass](./rendering-pass-recording.md).

`MaterialInstance`/`ComputeInstance` (`BindingInstance<T>`) also take an existing `Buffer`/`DynamicBuffer` directly, via `.with_buffer(name, buffer)`/`.with_dynamic_buffer(name, buffer)` — see [Materials](./materials.md#supplying-bind-group-values-materialinstance).
