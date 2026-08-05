# Samplers

Not built per-use — [`SamplerKind`](../src/wgpu/samplers.rs) picks from a small pre-built cache ([`GlobalSamplers`](../src/wgpu/samplers.rs), set up automatically by `WGPUPlugin`) rather than creating a new sampler per material instance:

```rust
BindingInstanceEntry::Sampler(SamplerKind::LinearRepeat)  // in an instance's params — see Materials
samplers.get(SamplerKind::LinearClamp)                     // -> &Sampler, e.g. for a hand-built BindGroupBuilder
```

Variants: `LinearRepeat`, `LinearClamp`, `LinearClampNoMip`, `Nearest`, `NearestClampBorder` (falls back to edge-clamping on web — WebGPU has no border color), `CompareLess` (shadow-map `textureSampleCompare`, paired with `BindingKind::comparison_sampler` — see [Bind Groups and Layouts](./bind-groups.md#a-bind-group)).

Samplers are cheap to share and there's rarely a reason to want more configurations than these — every `SamplerKind` variant is built once, eagerly, the first time `GlobalSamplers` is constructed, and reused for the lifetime of the app.
