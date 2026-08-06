# Custom GPU Resources

Anything one-off that needs the device before it can be built and isn't a material/mesh/texture — a camera, a depth buffer — is a [`LazyResource`](./the-asset-pipeline.md#lazyresourceb-exactly-one-constructed-on-demand), constructed once `WGPUBackend` (or your own `Deps`) exists, built entirely from the opaque builders covered elsewhere in this book. There's no built-in `pebble::wgpu` camera type — a camera's uniform layout is yours to define — so this page walks through building one by hand.

## The depth texture

A depth buffer has no source data to upload — it's not what [`Texture`](./textures.md) is for (that loads pixel data from a file/bytes). [`TextureBuilder`](./textures.md#a-render-target--depth-buffer-no-source-data) is the builder for exactly this: an empty GPU-side texture, handed back as an opaque `TextureView` ready to use as a render target:

```rust
use pebble::wgpu::prelude::*;

struct DepthTexture {
    view: TextureView,
}

impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let view = TextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
            .label("depth")
            .usage(TextureUsages::RENDER_ATTACHMENT)
            .build(backend);
        Some(DepthTexture { view })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, DepthTexture>::new())
```

## The camera

A camera needs a uniform buffer (the view/projection matrices), a bind group layout describing that buffer, and a bind group binding the two together — all built once the device exists. `wgpu::prelude` (imported above, alongside `WGPUBackend`) is where the builders below live — `BindGroupLayoutBuilder`, `BufferBuilder`, `BindGroupBuilder` — reach for those over hand-writing a `wgpu::BufferDescriptor`/`BindGroupLayoutDescriptor`/`BindGroupDescriptor` by hand. Every value that comes back — `BindGroupLayout`, `Buffer`, `BindGroup` — is opaque, the same as everywhere else in `pebble::wgpu`: no `wgpu::*` type anywhere in `Camera`'s own definition.

```rust
struct Camera {
    buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        // Same BindGroupLayoutBuilder that Material/Compute use internally
        // (see Materials and Compute Pipelines) — a camera's
        // layout isn't going through build_material, but there's no reason to
        // hand-write a wgpu::BindGroupLayoutDescriptor when the same builder
        // already covers a single uniform-buffer entry.
        let bind_group_layout = BindGroupLayoutBuilder::new()
            .label("camera_layout")
            .entry("camera", 0, BindingKind::uniform_buffer(ShaderStages::VERTEX))
            .build(backend);

        // Empty for now — there's no view/projection data yet to seed it
        // with; written every frame via Buffer::write once the actual
        // matrices are known (see below).
        let size = std::mem::size_of::<[[f32; 4]; 4]>() as u64 * 2; // view + projection
        let buffer = BufferBuilder::empty(size).label("camera").uniform().build(backend);

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .label("camera_bind_group")
            .buffer(&buffer)
            .build(backend);

        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, Camera>::new())
```

Updating it every frame is an ordinary `Update`-stage system, writing fresh matrices via `camera.buffer.write(&bytes)` — nothing new relative to the [buffer basics](./buffers.md).

## Wiring the camera into a material's pipeline layout

A camera is exactly the kind of thing [`GlobalLayoutPool`](./bind-groups.md#a-pool-of-shared-layouts) is for — shared across every material that needs it, rather than wired in by hand at each call site. Register it once, as soon as `Camera` exists:

```rust
fn register_camera_layout(camera: Res<Camera>, mut pool: ResMut<GlobalLayoutPool>) -> Option<()> {
    pool.register("camera", camera.bind_group_layout.clone());
    Some(())
}
```

Then any material reaches for it by name via `GroupEntry::Global`, at whatever position matches the shader's `@group(N)`:

```rust
use pebble::wgpu::layout::GroupEntry;

fn setup(
    // ...
    depth: Res<DepthTexture>,
) -> Option<()> {
    let material = Material::new(SHADER)
        // ... label, vertex_layouts as usual ...
        .entries(vec![
            GroupEntry::Global("camera"),          // @group(0): resolved from the pool at upload time
            GroupEntry::Own(material_entries()),   // @group(1): albedo/sampler
        ])
        .depth(DepthStencilState {
            format: TextureFormat::Depth16Unorm,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Less),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        })
        .build_asset("lit", &mut materials);
    // ...
    Some(())
}
```

Notice `setup` no longer takes `Res<Camera>` at all — `GroupEntry::Global("camera")` doesn't resolve until the material actually uploads, by which point `GPUMaterial::upload`'s own `Deps` (`Res<GlobalLayoutPool>`, wired up automatically) has whatever `register_camera_layout` has registered so far. If `"camera"` isn't registered yet, upload just returns `None` and retries next tick — the same "not ready" convention as every other dependency in this book — so `register_camera_layout` and `setup` can run in either order, even the same tick, without `setup` needing to know anything about `Camera` at all. Position in `.entries(...)` still *is* the `@group(N)` index, so `"camera"` landing at `@group(0)` here is just "it's the first element" — no separate group number to keep in sync with the shader by hand. `build_material` still panics on more than one `GroupEntry::Own` or on exceeding the device's `max_bind_groups`, turning either mistake into an immediate, specific error instead of an opaque wgpu validation failure at draw time.

For a layout that isn't going through the pool — a one-off not meant to be shared, say — `GroupEntry::Layout(bind_group_layout)` takes an already-built `BindGroupLayout` directly at whatever position you place it, no name or registration involved.

## Rendering with a depth attachment

`render_context` is a shortcut for "one color attachment, no depth." A depth pass uses `begin_pass` directly — see [Recording a Render Pass](./rendering-pass-recording.md#a-custom-render-target-or-depth-attachment):

```rust
use pebble::prelude::{ColorTarget, DepthTarget, Pass};

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    camera: Res<Camera>,
    depth: Res<DepthTexture>,
    // ... materials, meshes, instances as usual ...
) {
    let Some(mut active) = frame.active() else { return };
    let mut pass = active.begin_pass(Pass {
        colors: &[ColorTarget::default([0.2, 0.3, 0.3, 1.0])],
        depth: Some(DepthTarget::new(&depth.view, 1.0)),
    });

    pass.set_bind_group(1, &camera.bind_group, &[]); // group 1: shared across every draw

    for /* ... */ {
        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, &instance.bind_group, &[]); // group 0: per-instance
        // set_vertex_buffer / set_index_buffer / draw_indexed as usual
    }
}
```

`DepthTarget::new(view, 1.0)` clears the depth buffer to the far plane (`1.0`) at the start of the pass — a fragment only writes if its depth compares `Less` than what's already there, so nearer geometry always wins regardless of draw order. Bind group 1 (the camera) is set once per pass, outside the loop, since every draw shares the same view/projection; bind group 0 (the material instance) is set per-draw, inside it.

This same pattern — a `LazyResource` wrapping opaque builders, wired into a material via `GroupEntry::Global`/`GlobalLayoutPool` (or `GroupEntry::Layout` directly, for something not meant to be shared) — is how any one-off GPU resource gets built: a shadow-map pass's own uniform buffer, a global lighting bind group, anything that's "exactly one of, needs the device to exist first."
