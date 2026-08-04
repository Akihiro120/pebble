# Compute Pipelines

Compute passes reuse almost everything from Chapters 6–9: `ComputeDescriptor` mirrors `MaterialDescriptor`, `ComputeInstanceDescriptor` mirrors `MaterialInstanceDescriptor`, and both share the same `BindingKind`/`BindingEntry` vocabulary — the only real difference is shader stage.

This chapter builds a compute pass that doubles every number in a buffer, entirely off the render loop — dispatch happens from an ordinary system, not `SystemStage::Render`.

## The shader

```rust
const COMPUTE_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> data: array<f32>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * 2.0;
}
"#;
```

## Declaring the binding

Compute entries use the same `BindingKind` constructors as a material's — `storage_buffer_read_write` this time, visible to exactly the compute stage:

```rust
use pebble::wgpu::prelude::*;

fn compute_entries() -> Vec<BindingEntry> {
    vec![BindingEntry {
        name: "data",
        binding: 0,
        kind: BindingKind::storage_buffer_read_write(wgpu::ShaderStages::COMPUTE),
    }]
}
```

`build_compute` panics if an entry here isn't visible to *exactly* `COMPUTE` — reusing a material's `FRAGMENT`-visible entry by mistake fails loudly here instead of misbehaving silently.

## Setup: the pass and its buffer

```rust
use pebble::wgpu::{
    compute::ComputeDescriptor,
    instance::{BindingInstanceEntry, ComputeInstanceDescriptor},
};

fn setup(
    mut commands: Commands,
    mut computes: ResMut<Assets<ComputeDescriptor<'static>>>,
    mut instances: ResMut<Assets<ComputeInstanceDescriptor>>,
) -> Option<()> {
    let pass = computes.insert(
        "double",
        ComputeDescriptor {
            label: Some("double"),
            shader_source: COMPUTE_SHADER,
            entry_point: Some("cs_main"),
            entries: compute_entries(),
            ..Default::default()
        },
    );

    let numbers: Vec<f32> = (0..64).map(|i| i as f32).collect();
    let bytes = bytemuck::cast_slice(&numbers).to_vec();

    let instance = instances.insert(
        "double_instance",
        ComputeInstanceDescriptor::new(pass.id, vec![("data", BindingInstanceEntry::Storage(bytes))]),
    );

    commands.spawn((instance,));
    Some(())
}
```

`BindingInstanceEntry::Storage(bytes)` allocates and owns the storage buffer itself, sized from the initial bytes — the same instance mechanism from Chapter 9, just with `Storage` instead of `Texture`. Nothing here is compute-specific: `ComputeInstanceDescriptor` is a type alias for the exact same generic `GPUBindingInstance<T>` that backs `MaterialInstanceDescriptor`, with `T = GPUCompute` instead of `T = GPUMaterial`.

## Dispatching

There's no `FrameOperations`-mediated path for compute — a render pass is tied to an acquired frame, but a compute pass isn't tied to a frame at all, so dispatch happens directly against `backend.device`/`backend.queue`, from whatever system decides it's time to run:

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

        let mut encoder = backend.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("double-encoder"),
        });
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("double-pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&pass.pipeline);
            compute_pass.set_bind_group(0, Some(&instance.bind_group), &[]);
            compute_pass.dispatch_workgroups(1, 1, 1);
        }
        backend.queue.submit(Some(encoder.finish()));
    }
}
```

64 elements, one workgroup of 64 threads (matching `@workgroup_size(64)` in the shader), so a single `dispatch_workgroups(1, 1, 1)` covers the whole buffer.

## Reading the result back

The storage buffer now holds doubled values on the GPU — getting them back to the CPU is exactly [the async readback pattern from Chapter 5](./ch05-async.md#the-friendliest-option-asynceventwritert): `WGPUBackend::readback_buffer` returns a future, `AsyncEventWriter<T>` delivers its result as an ordinary event once it resolves. The one addition here is finding the right `wgpu::Buffer` to read from — `GPUBindingInstance::update`'s docs note the same buffers are addressable by name; a small accessor on your own code (or extending `GPUComputeInstance` usage to keep the handle around) gets you the `&wgpu::Buffer` to pass to `readback_buffer`. Nothing about the readback itself differs from the GPU→CPU example already covered.
