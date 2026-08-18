# Samplers

Pebble doesn't build a sampler per texture — instead, `BuiltinAssetsPlugin` (part of `GraphicsPlugin`) builds a fixed set of ready-made samplers once at startup, inserted as `GlobalSamplers`:

```rust,ignore
fn bind(samplers: Read<GlobalSamplers>) -> &Sampler {
    samplers.get(SamplerKind::LinearClamp)
}
```

`SamplerKind` variants:

| Variant | Filter | Address mode |
|---|---|---|
| `LinearRepeat` | linear, mipped | repeat |
| `LinearClamp` | linear, mipped | clamp to edge |
| `LinearClampNoMip` | linear, no mip sampling | clamp to edge |
| `Nearest` | nearest | repeat |
| `NearestClampBorder` | nearest | clamp to border (falls back to clamp-to-edge on wasm, where border color isn't supported) |
| `LinearClampBorder` | linear, mipped | clamp to border (falls back to clamp-to-edge on wasm, where border color isn't supported) |
| `CompareLess` | linear | clamp to edge, with a `Less` depth comparison — for shadow map PCF |

Pass a `SamplerKind` to `Material`/`Compute`'s `.sampler(name, kind)` (streamlined) or `.with_sampler(name, kind)` (value-only, see [Materials](./materials.md)) or `BindGroupBuilder::with_sampler` directly.

## `NearestClampBorder`/`LinearClampBorder` and device features

On native, `NearestClampBorder` and `LinearClampBorder` need the `ADDRESS_MODE_CLAMP_TO_BORDER` [device feature](./apps-and-plugins.md#gpu-device-features) — `GraphicsPlugin::new()` doesn't request it by default. If it wasn't requested (and granted), both fall back to plain `ClampToEdge` addressing (with a `tracing::warn!` at startup) instead of failing GPU validation. Request the feature explicitly if you need an actual border color:

```rust,ignore
App::new()
    .add_plugin(GraphicsPlugin::with_features(DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER))
    .run();
```

wasm builds don't need this — both fall back to clamp-to-edge there regardless of requested features, since WebGPU has no border-color support.

Either way, the border color itself is fixed to opaque white (`wgpu::SamplerBorderColor::OpaqueWhite`) — `wgpu` doesn't expose an arbitrary custom border color like desktop GL's `glTexParameterfv(GL_TEXTURE_BORDER_COLOR, ...)`.
