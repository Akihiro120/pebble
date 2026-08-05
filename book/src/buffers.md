# Buffers

Every buffer type here is opaque — there's no way to reach a raw `wgpu::Buffer` from outside the crate. Binding one into a bind group goes through [`BindGroupBuilder`](./bind-groups.md#a-bind-group) directly; writing to one is a method call, not a `queue.write_buffer(...)` you have to thread a queue reference to.

## A plain, uniform, or storage buffer

[`BufferBuilder`](../src/wgpu/buffers.rs) — empty (write into it later) or pre-populated. Takes `&WGPUBackend`, not just a device, since the resulting [`Buffer`](../src/wgpu/buffer.rs) caches its own queue access:

```rust
// Empty, written into later via `.write()`.
let camera_buffer = BufferBuilder::new().label("camera").uniform().size(64).build(&backend);

// Pre-populated.
let vertex_buffer = BufferBuilder::new()
    .label("mesh vertices")
    .usage(BufferUsages::VERTEX)
    .data(bytemuck::cast_slice(&vertices))
    .build(&backend);

// Later, any time:
camera_buffer.write(&new_matrix_bytes);              // whole buffer, offset 0
camera_buffer.write_at(offset, &partial_bytes);       // starting at a byte offset
```

`.uniform()`/`.storage()` are shorthand for the usual `UNIFORM | COPY_DST`/`STORAGE | COPY_DST` flag pairs; use `.usage(...)` directly for anything else (vertex/index buffers, a `MAP_READ` staging buffer, an `INDIRECT` buffer — see [Indirect Draws](./rendering-pass-recording.md#indirect-draws)).

## A dynamically-offset buffer (many elements, one buffer)

[`DynamicBufferBuilder`](../src/wgpu/buffers.rs) — sized and aligned for `count` elements of `element_size` bytes, selected later via `set_bind_group`'s dynamic offset. Returns a [`DynamicBuffer`](../src/wgpu/buffer.rs) bundling the buffer with its own stride and element size, so neither can drift out of sync with what it was actually built with:

```rust
let dynamic = DynamicBufferBuilder::uniform(element_size, count).build(&backend);
// ... later, per element:
dynamic.write_element(index, &element_bytes);
// ... at draw time:
pass.set_bind_group(0, &bind_group, &[index as u32 * dynamic.stride() as u32]);
```

Pair with [`BindingKind::dynamic_uniform_buffer`](./bind-groups.md#a-bind-group-layout) for the layout and [`BindGroupBuilder::dynamic_buffer`](./bind-groups.md#a-bind-group) for the bind group — a large pool of per-object data (transforms, materials) selected by offset instead of one bind group per object.

## Reading a buffer back to the CPU

`Buffer::read()`/`read_as::<T>()` copy the buffer's current contents back, resolving asynchronously — the same [async pattern](./async-and-background-tasks.md#the-friendliest-option-asynceventwritert) used for any other background result:

```rust
fn start_readback(events: AsyncEventWriter<ReadbackDone>, buffer: Res<SomeGpuBuffer>) {
    let future = buffer.0.read(); // buffer.0: pebble::wgpu::buffer::Buffer
    events.spawn(async move { ReadbackDone(future.await) });
}
```

Works identically on native and web — see [Async Systems and Background Tasks](./async-and-background-tasks.md#web-support-at-a-glance).
