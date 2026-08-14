# Compute Pipelines

`Compute` is the compute-pipeline counterpart to [`Material`](./materials.md) — WGSL shader source plus its bind group layout, same asset pipeline pattern:

```rust,ignore
fn setup(backend: Read<Backend>, mut computes: Write<Assets<Compute>>) {
    let entries = OwnEntriesBuilder::new()
        .with_entry("data", BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE))
        .build();

    Compute::new(SHADER_SOURCE)
        .with_entries(vec![entries])
        .build_asset("particles", &mut computes);
}
```

Every entry in a compute pipeline's own bind group must be visible to exactly `ShaderStages::COMPUTE` — mixing in `VERTEX`/`FRAGMENT` panics at upload time. `with_entry_point` overrides the default `cs_main`.

Supply the actual buffers/textures the same way as a material, via `ComputeInstance` (`BindingInstance<Compute>`):

```rust,ignore
ComputeInstance::new(compute_handle)
    .with_storage("data", initial_bytes)
    .build_asset("particles_instance", &mut instances);
```

`.with_storage`/`.with_uniform` build a fresh buffer from raw bytes. To bind a buffer you already have — e.g. chaining compute passes, where one pass's output storage buffer is the next pass's input — use `.with_buffer(name, existing_buffer)` instead: it binds `existing_buffer` as-is, no new buffer is created. `existing_buffer` must already carry usage flags matching how `name` was declared in `with_entries` (`BufferUsages::UNIFORM` or `::STORAGE`):

```rust,ignore
ComputeInstance::new(compute_handle)
    .with_buffer("data", previous_pass_output.clone())
    .build_asset("second_pass_instance", &mut instances);
```

## Dispatching

Unlike rendering, compute work isn't tied to the frame lifecycle — it doesn't need a swapchain frame to exist, so it dispatches immediately via `Backend::dispatch_compute`, in its own command encoder, submitted right away:

```rust,ignore
fn simulate(backend: Read<Backend>, compute: Read<Assets<Compute>>, instances: Read<Assets<ComputeInstance>>) {
    let Some(pipeline) = compute.get(compute_handle) else { return };
    let Some(instance) = instances.get(instance_handle) else { return };

    backend.dispatch_compute(|pass| {
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &instance.bind_group, &[]);
        pass.dispatch_workgroups(64, 1, 1);
    });
}
```

`ComputePass` also has `dispatch_workgroups_indirect(buffer, offset)`. To read a result back afterward, use [`Buffer::read`](./buffers.md#reading-a-buffer-back) on the underlying storage buffer and poll the returned `Promise` — the GPU work itself is already submitted by the time `dispatch_compute` returns, but reading it back is still async.
