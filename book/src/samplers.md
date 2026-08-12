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
