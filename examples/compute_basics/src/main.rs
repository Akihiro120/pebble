//! Minimal `pebble::wgpu` compute example: dispatch a compute shader that
//! doubles every number in a buffer, then read the result back to the CPU
//! and print it. Opens a window (a `WGPUBackend` needs one to exist), but
//! nothing in this example is drawn with it — the window just clears to a
//! solid color while the compute pass runs once in the background.

use pebble::prelude::*;
use pebble::wgpu::prelude::*;
use pebble::wgpu::{
    backend::{WGPUBackend, WGPUPlugin},
    compute::{Compute, GPUCompute},
    instance::{BindingInstanceEntry, ComputeInstance, GPUComputeInstance},
};

const COMPUTE_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> data: array<f32>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * 2.0;
}
"#;

struct DoubleResult(Vec<f32>);

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_plugin(WGPUPlugin::new(WindowConfig {
            title: "Compute Basics".to_string(),
            width: 800,
            height: 600,
        }))
        .add_plugin(BackgroundTasksPlugin::new(2))
        .add_async_event::<DoubleResult>()
        .add_system(SystemStage::PreUpdate, setup.once())
        .add_system(SystemStage::Update, dispatch.once())
        .add_system(SystemStage::Update, on_result)
        .add_system(SystemStage::Render, render)
        .build()
        .run();
}

fn setup(
    mut commands: Commands,
    mut computes: ResMut<Assets<Compute>>,
    mut instances: ResMut<Assets<ComputeInstance>>,
) -> Option<()> {
    let pass = Compute::new(COMPUTE_SHADER)
        .label("double")
        .entry_point("cs_main")
        .entries(vec![BindingEntry {
            name: "data",
            binding: 0,
            kind: BindingKind::storage_buffer_read_write(ShaderStages::COMPUTE),
        }])
        .build_asset("double", &mut computes);

    let numbers: Vec<f32> = (0..64).map(|i| i as f32).collect();
    let bytes = bytemuck::cast_slice(&numbers).to_vec();

    let instance = ComputeInstance::new(pass.id, vec![("data", BindingInstanceEntry::Storage(bytes))])
        .build_asset("double_instance", &mut instances);

    commands.spawn((instance,));
    Some(())
}

/// Dispatches once the compute pass/instance have both finished uploading,
/// then kicks off the async readback. `.once()` retries this every tick
/// until the `?`s all succeed — same "not ready yet" shape as everything
/// else in the asset pipeline.
fn dispatch(
    backend: Res<WGPUBackend>,
    computes: Res<ProcessedAssets<GPUCompute>>,
    instances: Res<ProcessedAssets<GPUComputeInstance>>,
    events: AsyncEventWriter<DoubleResult>,
    mut query: Query<&Handle<ComputeInstance>>,
) -> Option<()> {
    let instance_handle = query.iter().next()?;
    let instance = instances.get(instance_handle.id)?;
    let pass = computes.get(instance.target)?;

    let mut encoder = backend.create_command_encoder(Some("double-encoder"));
    {
        let mut compute_pass = encoder.compute_pass(Some("double-pass"));
        compute_pass.set_pipeline(&pass.pipeline);
        compute_pass.set_bind_group(0, &instance.bind_group, &[]);
        compute_pass.dispatch_workgroups(1, 1, 1);
    }
    backend.submit(encoder);

    let buffer = instance.buffer("data")?;
    let future = buffer.read_as::<f32>();
    events.spawn(async move { DoubleResult(future.await) });

    Some(())
}

fn on_result(mut reader: EventReader<DoubleResult>) {
    for DoubleResult(values) in reader.iter() {
        println!("doubled: {values:?}");
    }
}

fn render(mut frame: ResMut<CurrentFrame<WGPUBackend>>) {
    if let Some(mut active) = frame.active() {
        let _pass = active.render_context([0.05, 0.05, 0.08, 1.0]);
    }
}
