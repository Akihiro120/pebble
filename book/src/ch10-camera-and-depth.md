# Camera, Depth, and Lazy Resources

> This chapter is illustrative rather than copy-pasteable: there's no built-in `pebble::wgpu` camera or depth type (there isn't meant to be — a camera's uniform layout is yours to define), so the code below follows the exact same `LazyResource`/`extra_layouts` mechanism as the previous two chapters, adapted from the fully working, tested [`orbit_camera`](https://github.com/Akihiro120/pebble/tree/main/examples/orbit_camera) example. That example targets a hand-rolled `Backend` rather than `pebble::wgpu::backend::WGPUBackend`, but every API used below — `LazyResource`, `MaterialDescriptor::extra_layouts`, `begin_pass` — is identical either way. Read `orbit_camera`'s README for the complete, runnable version.

A depth buffer and a camera are both things Chapter 6 called out as good `LazyResource` candidates: exactly one instance, needs the GPU device to exist before it can be built, not authored data.

## The depth texture

A depth buffer has no source data to upload — it's not what [`TextureDescriptor`](./ch09-textures.md) is for (that loads pixel data from a file/bytes). [`TextureBuilder`](../src/wgpu/texture_view.rs) is the builder for exactly this: an empty GPU-side texture, handed back as an opaque [`TextureView`](../src/wgpu/texture_view.rs) ready to use as a render target:

```rust
use pebble::wgpu::prelude::*;

struct DepthTexture {
    view: TextureView,
}

impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let view = TextureBuilder::new(backend.config.width, backend.config.height, wgpu::TextureFormat::Depth16Unorm)
            .label("depth")
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT)
            .build(backend);
        Some(DepthTexture { view })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, DepthTexture>::new())
```

## The camera

A camera needs a uniform buffer (the view/projection matrices), a bind group layout describing that buffer, and a bind group binding the two together — all built once the device exists. `wgpu::prelude` (imported above, alongside `WGPUBackend`) is where the builders below live — `BindGroupLayoutBuilder`, `BufferBuilder`, `BindGroupBuilder` — reach for those over hand-writing a `wgpu::BufferDescriptor`/`BindGroupLayoutDescriptor`/`BindGroupDescriptor` against `backend.device` yourself. Every value that comes back — `BindGroupLayout`, `Buffer`, `BindGroup` — is opaque, the same as everywhere else in `pebble::wgpu`:

```rust
struct Camera {
    buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        // Same `BindGroupLayoutBuilder` that `MaterialDescriptor`/
        // `ComputeDescriptor` use internally (see Chapters 8 and 11) — a
        // camera's layout isn't going through `build_material`, but
        // there's no reason to hand-write a `wgpu::BindGroupLayoutDescriptor`
        // when the same builder already covers a single uniform-buffer entry.
        let bind_group_layout = BindGroupLayoutBuilder::new()
            .label("camera_layout")
            .entry("camera", 0, BindingKind::uniform_buffer(wgpu::ShaderStages::VERTEX))
            .build(&backend.device);

        // Empty for now — there's no view/projection data yet to seed it
        // with; written every frame via `Buffer::write` once the actual
        // matrices are known (see below).
        let size = std::mem::size_of::<[[f32; 4]; 4]>() as u64 * 2; // view + projection
        let buffer = BufferBuilder::new()
            .label("camera")
            .uniform()
            .size(size)
            .build(backend);

        let bind_group = BindGroupBuilder::new(&bind_group_layout)
            .label("camera_bind_group")
            .buffer(&buffer)
            .build(&backend.device);

        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, Camera>::new())
```

Updating it every frame is an ordinary `Update`-stage system, writing fresh matrices via `camera.buffer.write(&bytes)` — nothing new relative to Part I.

## Wiring the camera into the material's pipeline layout

The quad material from Chapter 9 occupies `@group(0)` for its own texture/sampler bind group. The camera needs its own group too — `MaterialDescriptor::extra_layouts` is exactly for bind group layouts that exist *outside* a material's own `entries`:

```rust
use pebble::wgpu::layout::OwnedGroupLayout;

fn setup(
    // ...
    camera: Res<Camera>,
    depth: Res<DepthTexture>,
) -> Option<()> {
    let material = materials.insert("lit", MaterialDescriptor {
        // ... shader_source, vertex_layouts, entries (albedo/sampler at @group(0)) as before ...
        extra_layouts: vec![OwnedGroupLayout { group: 1, layout: camera.bind_group_layout.clone() }],
        depth: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth16Unorm,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        ..Default::default()
    });
    // ...
    Some(())
}
```

`OwnedGroupLayout::layout` takes the same opaque `BindGroupLayout` `bind_group_layout` above — `Clone` because a camera's layout might be wired into more than one material, and it's the same cheap `Arc`-backed handle underneath either way. `own_group` (the material's own texture/sampler entries, `Some(0)`) plus every group in `extra_layouts` must cover `0..=max` exactly once — building the pipeline layout panics on a gap or a collision, turning a mismatched `@group(N)` in the shader into an immediate, specific error instead of an opaque wgpu validation failure at draw time.

`setup` requiring `Res<Camera>`/`Res<DepthTexture>` (hard requirements, from Chapter 2) is what makes this correct without any manual waiting: `setup` itself won't run until both lazy resources exist, so by the time it builds `MaterialDescriptor`, `camera.bind_group_layout` is guaranteed to be real.

## Rendering with a depth attachment

`render_context` (used in every earlier chapter) is a shortcut for "one color attachment, no depth." A depth pass uses `begin_pass` directly:

```rust
use pebble::prelude::{ColorTarget, DepthTarget, Pass};

fn render(
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    camera: Res<Camera>,
    depth: Res<DepthTexture>,
    // ... materials, meshes, instances as before ...
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
        // set_vertex_buffer / set_index_buffer / draw_indexed as before
    }
}
```

`DepthTarget::new(view, 1.0)` clears the depth buffer to the far plane (`1.0`) at the start of the pass — a fragment only writes if its depth compares `Less` than what's already there, so nearer geometry always wins regardless of draw order. Bind group 1 (the camera) is set once per pass, outside the loop, since every draw shares the same view/projection; bind group 0 (the material instance) is set per-draw, inside it.
