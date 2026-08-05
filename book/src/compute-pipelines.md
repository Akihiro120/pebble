# Compute Pipelines

Compute passes reuse almost everything from materials: `ComputeDescriptor` mirrors `MaterialDescriptor`, `ComputeInstanceDescriptor` mirrors `MaterialInstanceDescriptor`, and both share the same `BindingKind`/`BindingEntry` vocabulary (see [Bind Groups and Layouts](./bind-groups.md)) — the only real difference is shader stage: a compute entry must be visible to *exactly* `COMPUTE`, not `FRAGMENT`/`VERTEX`.

## Declaring the binding

```rust
use pebble::wgpu::binding::{BindingEntry, BindingKind};

fn compute_entries() -> Vec<BindingEntry> {
    vec![BindingEntry {
        name: "data",
        binding: 0,
        kind: BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE),
    }]
}
```

`build_compute` panics if an entry here isn't visible to *exactly* `COMPUTE` — reusing a material's `FRAGMENT`-visible entry by mistake fails loudly here instead of misbehaving silently.

## Building a compute pass and its instance

[`ComputeDescriptor`](../src/wgpu/compute.rs) — same `entries`/`own_group`/`extra_layouts` shape as `MaterialDescriptor`:

```rust
use pebble::wgpu::compute::ComputeDescriptor;

let pass = computes.insert("double", ComputeDescriptor {
    label: Some("double"),
    shader_source: COMPUTE_SHADER,
    entry_point: Some("cs_main"),
    entries: compute_entries(),
    ..Default::default()
});
```

[`ComputeInstanceDescriptor`](../src/wgpu/instance.rs) — same type as `MaterialInstanceDescriptor` (`GPUBindingInstance<T>` generic over the target, see [Materials](./materials.md#a-material-instance-concrete-resources-bound-to-a-material)), just `T = GPUCompute`:

```rust
use pebble::wgpu::instance::{BindingInstanceEntry, ComputeInstanceDescriptor};

let numbers: Vec<f32> = (0..64).map(|i| i as f32).collect();
let bytes = bytemuck::cast_slice(&numbers).to_vec();

let instance = instances.insert("double_instance", ComputeInstanceDescriptor::new(
    pass.id,
    vec![("data", BindingInstanceEntry::Storage(bytes))],
));
```

`BindingInstanceEntry::Storage(bytes)` allocates and owns the storage buffer itself, sized from the initial bytes.

## Dispatching

Dispatching isn't `FrameOperations`-mediated — a compute pass isn't tied to an acquired frame the way a render pass is (it can run from any system, not just one on `SystemStage::Render`) — but it's just as opaque. `WGPUBackend::create_command_encoder`/`CommandEncoder::compute_pass`/`WGPUBackend::submit` cover standalone dispatch the same opaque way `begin_pass` covers rendering:

```rust
use pebble::wgpu::compute::GPUCompute;
use pebble::wgpu::instance::GPUComputeInstance;

fn dispatch(
    backend: Res<WGPUBackend>,
    computes: Res<ProcessedAssets<GPUCompute>>,
    instances: Res<ProcessedAssets<GPUComputeInstance>>,
    mut query: Query<&Handle<ComputeInstanceDescriptor>>,
) {
    for instance_handle in query.iter() {
        let Some(instance) = instances.get(instance_handle.id) else { continue };
        let Some(pass) = computes.get(instance.target) else { continue };

        let mut encoder = backend.create_command_encoder(Some("double-encoder"));
        {
            let mut compute_pass = encoder.compute_pass(Some("double-pass"));
            compute_pass.set_pipeline(&pass.pipeline);
            compute_pass.set_bind_group(0, &instance.bind_group, &[]);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }
        backend.submit(encoder);
    }
}
```

64 elements, one workgroup of 64 threads (matching `@workgroup_size(64)` in the shader), so a single `dispatch_workgroups(1, 1, 1)` covers the whole buffer.

## Indirect dispatch

`compute_pass.dispatch_workgroups_indirect(&indirect_buffer, offset)` reads a `DispatchIndirectArgs { x, y, z }` from `indirect_buffer` at `offset` instead of a CPU-known workgroup count — the compute-side equivalent of [indirect draws](./rendering-pass-recording.md#indirect-draws), for a dispatch size the GPU itself computed.

## Reading the result back

The storage buffer now holds computed values on the GPU — getting them back to the CPU is exactly [the async readback pattern](./async-and-background-tasks.md#the-friendliest-option-asynceventwritert): `Buffer::read`/`read_as::<T>` returns a future, `AsyncEventWriter<T>` delivers its result as an ordinary event once it resolves. The one addition here is finding the right buffer to read from — `GPUBindingInstance::buffer(name)` returns the same owned `Buffer` `update` writes to, by the same name:

```rust
fn start_readback(events: AsyncEventWriter<DoubleResult>, instances: Res<ProcessedAssets<GPUComputeInstance>>, query: Query<&Handle<ComputeInstanceDescriptor>>) {
    let Some(instance_handle) = query.iter().next() else { return };
    let Some(instance) = instances.get(instance_handle.id) else { return };
    let Some(buffer) = instance.buffer("data") else { return };

    let future = buffer.read_as::<f32>();
    events.spawn(async move { DoubleResult(future.await) });
}
```

Nothing about the readback itself differs from the GPU→CPU example already covered in [Async Systems and Background Tasks](./async-and-background-tasks.md).
