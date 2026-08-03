# Camera, Depth, and Lazy Resources

> This chapter is illustrative rather than copy-pasteable: there's no built-in `pebble::wgpu` camera or depth type (there isn't meant to be — a camera's uniform layout is yours to define), so the code below follows the exact same `LazyResource`/`extra_layouts` mechanism as the previous two chapters, adapted from the fully working, tested [`orbit_camera`](https://github.com/Akihiro120/pebble/tree/main/examples/orbit_camera) example. That example targets a hand-rolled `Backend` rather than `pebble::wgpu::backend::WGPUBackend`, but every API used below — `LazyResource`, `MaterialDescriptor::extra_layouts`, `begin_pass` — is identical either way. Read `orbit_camera`'s README for the complete, runnable version.

A depth buffer and a camera are both things Chapter 6 called out as good `LazyResource` candidates: exactly one instance, needs the GPU device to exist before it can be built, not authored data.

## The depth texture

```rust
use pebble::wgpu::backend::WGPUBackend;

struct DepthTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl LazyResource<WGPUBackend> for DepthTexture {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let texture = backend.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: backend.config.width, height: backend.config.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth16Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Some(DepthTexture { texture, view })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, DepthTexture>::new())
```

## The camera

A camera needs a uniform buffer (the view/projection matrices), a bind group layout describing that buffer, and a bind group binding the two together — all built once the device exists:

```rust
struct Camera {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl LazyResource<WGPUBackend> for Camera {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let buffer = backend.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as u64 * 2, // view + projection
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = backend.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = backend.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }],
        });

        Some(Camera { buffer, bind_group_layout, bind_group })
    }
}
```

```rust
.add_plugin(LazyResourcePlugin::<WGPUBackend, Camera>::new())
```

Updating it every frame is an ordinary `Update`-stage system, writing fresh matrices via `queue.write_buffer` — nothing new relative to Part I.

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
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        ..Default::default()
    });
    // ...
    Some(())
}
```

`own_group` (the material's own texture/sampler entries, `Some(0)`) plus every group in `extra_layouts` must cover `0..=max` exactly once — `assemble_bind_group_layouts` panics on a gap or a collision, turning a mismatched `@group(N)` in the shader into an immediate, specific error instead of an opaque wgpu validation failure at draw time.

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

    pass.set_bind_group(1, Some(&camera.bind_group), &[]); // group 1: shared across every draw

    for /* ... */ {
        pass.set_pipeline(&material.pipeline);
        pass.set_bind_group(0, Some(&instance.bind_group), &[]); // group 0: per-instance
        // set_vertex_buffer / set_index_buffer / draw_indexed as before
    }
}
```

`DepthTarget::new(view, 1.0)` clears the depth buffer to the far plane (`1.0`) at the start of the pass — a fragment only writes if its depth compares `Less` than what's already there, so nearer geometry always wins regardless of draw order. Bind group 1 (the camera) is set once per pass, outside the loop, since every draw shares the same view/projection; bind group 0 (the material instance) is set per-draw, inside it.
