# Compute Pipelines

`Compute` is the compute-pipeline counterpart to [`Material`](./materials.md) — WGSL shader source plus its bind group entries *and* the buffers/textures it dispatches with, all in one asset, same pattern:

```rust,ignore
fn setup(mut computes: Write<Assets<Compute>>) {
    Compute::new(SHADER_SOURCE)
        .storage("data", initial_bytes)   // read-write by default (compute usually writes what it binds)
        .build_asset("particles", &mut computes);
}
```

`.storage`/`.texture`/`.texture_array`/`.cubemap`/`.sampler`/`.uniform`/`.uniform_value`/`.storage_value` all work the same as `Material`'s — see [Materials](./materials.md#bind-group-values-streamlined-vs-manual) — except visibility is always exactly `ShaderStages::COMPUTE`; there's no visibility to choose. `with_entry_point` overrides the default `cs_main`.

Need an explicit binding index, or a bind group entry the streamlined calls don't produce (a non-default sample type, a dynamic-offset buffer)? Drop to `.with_entry`/`.with_entry_at` plus the matching value-only call, same two-step pattern as `Material`:

```rust,ignore
Compute::new(SHADER_SOURCE)
    .with_entry_at("data", 0, BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE))
    .with_storage("data", initial_bytes)
    .build_asset("particles", &mut computes);
```

To bind a buffer you already have — e.g. chaining compute passes, where one pass's output storage buffer is the next pass's input — use `.with_buffer(name, existing_buffer)` instead: it binds `existing_buffer` as-is, no new buffer is created. `existing_buffer` must already carry usage flags matching how `name` was declared (`BufferUsages::UNIFORM` or `::STORAGE`):

```rust,ignore
Compute::new(SHADER_SOURCE)
    .with_buffer("data", previous_pass_output.clone())
    .build_asset("second_pass", &mut computes);
```

Same [pipeline sharing](./materials.md#many-uniform-combinations-one-shader) as `Material` — several `Compute`s using the same shader and bind group shape compile once and share the result, via `ComputePipelineCache`.

## Dispatching

Unlike rendering, compute work isn't tied to the frame lifecycle — it doesn't need a swapchain frame to exist, so it dispatches immediately via `Backend::dispatch_compute`, in its own command encoder, submitted right away:

```rust,ignore
fn simulate(backend: Read<Backend>, computes: Read<Assets<Compute>>) {
    backend.dispatch_compute(|pass| {
        bind_comp!(pass, computes, compute_handle);
        pass.dispatch_workgroups(64, 1, 1);
    });
}
```

`ComputePass` also has `dispatch_workgroups_indirect(buffer, offset)`. To read a result back afterward, use [`Buffer::read`](./buffers.md#reading-a-buffer-back) on the underlying storage buffer and poll the returned `Promise` — the GPU work itself is already submitted by the time `dispatch_compute` returns, but reading it back is still async.

## `#[derive(ComputeParams)]`

Same idea as [`#[derive(MaterialParams)]`](./material-params-derive.md#computeparams) — a plain struct's fields become the `.storage(...)`/`.texture(...)`/etc. chain, minus the visibility question (always `COMPUTE`).
