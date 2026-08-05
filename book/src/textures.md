# Textures

## Loading a texture, array, or cubemap

Three descriptors, same `from_*` constructor pattern — decode/upload happens on the [asset pipeline](./the-asset-pipeline.md) like any other asset, no manual plugin registration needed:

```rust
use pebble::wgpu::{textures::TextureDescriptor, texture_array::TextureArrayDescriptor, cubemap::CubemapDescriptor};

textures.insert("brick", TextureDescriptor::from_file("assets/brick.png").with_mips());
arrays.insert("atlas", TextureArrayDescriptor::from_files(vec!["a.png", "b.png", "c.png"]));
cubemaps.insert("sky", CubemapDescriptor::from_files(1024, [
    "px.png", "nx.png", "py.png", "ny.png", "pz.png", "nz.png",
]));
```

`from_data`/`from_faces` variants take raw bytes instead of a file path, for procedurally-generated or embedded pixel data. `.with_mips()` (texture only) generates a full mip chain via `MipmapGenerator`. Uploads to `ProcessedAssets<GPUTexture>`/`GPUTextureArray`/`GPUCubemap` — all three opaque, no `texture`/`view` fields to reach in from outside the crate. Each exposes `.width()`/`.height()` (or `.size()` for a cubemap) and a `.write*()` to overwrite level-0 pixel data after upload:

```rust
brick_texture.write(&new_pixels);              // GPUTexture
atlas.write_layer(2, &new_pixels);              // GPUTextureArray, one layer
sky.write_face(0, &new_pixels);                 // GPUCubemap, one face (+X)
```

Binding one into a material instance goes through `BindingInstanceEntry::Texture`/`TextureArray`/`Cubemap` — see [Materials](./materials.md#a-material-instance-concrete-resources-bound-to-a-material).

## Texture formats

`TextureDescriptor`'s `format` isn't limited to a handful of formats — every regular 8/16/32-bit unorm/float format (`R8Unorm`, `Rg16Float`, `Bgra8Unorm`, `Rgba32Float`, ...) works for both `from_file` decoding and raw `from_data` uploads. Block-compressed formats (`Bc*`/`Etc2*`/`Astc`) aren't supported by either path yet — decoding one from an ordinary image file isn't possible this way regardless, and uploading pre-compressed bytes via `from_data` needs block-aware row-stride math this loader doesn't compute yet.

## Rendering into a cubemap face (environment capture)

[`GPUCubemap::face_attachment`](../src/wgpu/cubemap.rs) — a render-target [`TextureView`](#a-render-target--depth-buffer-no-source-data) onto one face (`0..=5`, `+X -X +Y -Y +Z -Z`) at one mip level, for a `CubemapDescriptor::empty()` cubemap (which sets `RENDER_ATTACHMENT` usage automatically). Capture a scene into all 6 faces, or write successive mip levels from a specular IBL prefilter pass:

```rust
for face in 0..6 {
    let view = cubemap.face_attachment(face, 0);
    let mut pass = active.begin_pass(Pass {
        colors: &[ColorTarget::Custom { attachment: &view, clear: Some([0.0, 0.0, 0.0, 1.0]) }],
        depth: None,
    });
    // ... render the scene from this face's view direction ...
}
```

## A render target / depth buffer (no source data)

[`TextureBuilder`](../src/wgpu/texture_view.rs) — for a one-off GPU-side texture with nothing to upload (a depth buffer, an off-screen render target), unlike `TextureDescriptor` above which always loads from a file/bytes through the asset pipeline. Hands back an opaque [`TextureView`](../src/wgpu/texture_view.rs) — the type `ActiveFrame::begin_pass`'s `ColorTarget::Custom`/`DepthTarget` expect:

```rust
let depth_view = TextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth16Unorm)
    .label("depth")
    .usage(TextureUsages::RENDER_ATTACHMENT)
    .build(backend);

// ... later, in a render system:
let mut pass = active.begin_pass(Pass {
    colors: &[ColorTarget::default([0.2, 0.3, 0.3, 1.0])],
    depth: Some(DepthTarget::new(&depth_view, 1.0)),
});
```

A depth texture (or any other render target) is almost always built once, in a [`LazyResource`](./the-asset-pipeline.md#lazyresourceb-exactly-one-constructed-on-demand) — see [Custom GPU Resources](./custom-gpu-resources.md) for the full pattern, including wiring a camera's bind group alongside it. Sampling a `TextureView` back in a later pass (a shadow map, a post-process input) goes through [`BindGroupBuilder::texture_view`](./bind-groups.md#a-bind-group), which needs `TEXTURE_BINDING` added to the usage above.
