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
| `CompareLess` | linear | clamp to edge, with a `Less` depth comparison — for shadow map PCF |

Pass a `SamplerKind` to `MaterialInstance::with_sampler`/`ComputeInstance::with_sampler` (see [Materials](./materials.md)) or `BindGroupBuilder::with_sampler` directly.

## `NearestClampBorder` and device features

On native, `NearestClampBorder` needs the `ADDRESS_MODE_CLAMP_TO_BORDER` [device feature](./apps-and-plugins.md#gpu-device-features) — `GraphicsPlugin::new()` doesn't request it by default. If it wasn't requested (and granted), `BuiltinAssetsPlugin` skips building that sampler at startup and logs a `tracing::error!` instead of failing GPU validation; calling `GlobalSamplers::get(SamplerKind::NearestClampBorder)` afterwards panics, same as any other missing kind. Request the feature explicitly if you need this sampler:

```rust,ignore
App::new()
    .add_plugin(GraphicsPlugin::with_features(DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER))
    .run();
```

wasm builds don't need this — `NearestClampBorder` falls back to clamp-to-edge there regardless of requested features, since WebGPU has no border-color support.
