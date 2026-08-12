# Textures

Three related asset types share the same construction pattern: `Texture` (2D), `TextureArray` (2D array, one file/buffer per layer), and `Cubemap` (six equal-size faces, wgpu's `+X, -X, +Y, -Y, +Z, -Z` order). This page covers `Texture`; the other two differ only in shape.

```rust,ignore
fn setup(backend: Read<Backend>, mut textures: Write<Assets<Texture>>) {
    let from_disk = Texture::from_file("assets/albedo.png")
        .with_mips()
        .build_asset("albedo", &mut textures);

    let from_pixels = Texture::from_data(256, 256, TextureFormat::Rgba8Unorm, pixel_bytes)
        .build_asset("noise", &mut textures);

    let render_target = Texture::empty(1920, 1080, TextureFormat::Rgba16Float)
        .build_asset("bloom_buffer", &mut textures);
}
```

- `from_file(path)` — decoded from disk on upload. Supports the regular 8/16/32-bit unorm and float formats, not block-compressed or multi-planar ones.
- `from_data(width, height, format, bytes)` — from an in-memory buffer you already have.
- `empty(width, height, format)` — no source data at all: a render target (post-processing, shadow maps), or something you'll `write()` yourself.
- `.with_mips()` — generates a full GPU-side mip chain. `.with_mip_count(n)` generates exactly `n` levels instead (e.g. for a PBR prefilter pass).

## CPU-side access

`data()` returns the CPU-side pixels — only ever `Some` for a `from_data()` texture, since `from_file()` re-decodes from disk on each upload rather than keeping a copy. `release_cpu_data()` frees that copy; unlike `Mesh`, a released texture can still be re-uploaded later, it just comes back empty instead of with its original contents.

## Writing into a texture at runtime

The uploaded `GPUTexture` has `write(mip_level, pixels)` to overwrite one mip level — for a texture you're streaming or rendering into from the CPU side — and `get_view(mip_level)` for binding a specific level (e.g. as a render target during mip generation).

## Texture arrays and cubemaps

```rust,ignore
TextureArray::from_files(vec!["a.png", "b.png", "c.png"]).build_asset("atlas", &mut arrays);
Cubemap::from_files(1024, [px, nx, py, ny, pz, nz]).build_asset("sky", &mut cubemaps);
```

`GPUTextureArray::write_layer`/`GPUCubemap::write_face` mirror `GPUTexture::write`, with an extra layer/face index. `get_view` similarly takes a layer or face alongside the mip level.
