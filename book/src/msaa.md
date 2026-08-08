# Multisampled Anti-Aliasing (MSAA)

Turning on MSAA for the window surface is a `WGPUBackend` setting, not something you wire into `Pass`/`ColorTarget` by hand — call it once at startup, before building any material that renders into the default target:

```rust
fn setup_msaa(mut backend: ResMut<WGPUBackend>) {
    backend.set_msaa(4); // 0 or 1 turns it back off
}
```

Once set, `ColorTarget::Default` (see [Recording a Render Pass](./rendering-pass-recording.md#a-custom-render-target-or-depth-attachment)) automatically renders into an internally-managed multisampled texture and resolves into the real surface — `WGPUBackend::resize` keeps that texture matched to the surface size, no extra wiring needed on your end.

## Opting materials and depth attachments in

Two things still need to opt in explicitly, because a single frame can legitimately mix sample counts (an MSAA scene pass, then a non-MSAA post-process/UI pass reading the resolved result — forcing every material to match one global count would break exactly that):

```rust
// A material meant to render into the (now-MSAA) default target — see Materials:
MaterialBuilder::new(SHADER)
    .sample_count(backend.sample_count()) // 1 (MaterialBuilder's default) means "not this pass"
    // ...
    .build_asset("lit", &mut materials);

// A depth attachment used alongside it needs the same sample count — see Textures:
RenderTargetTextureBuilder::new(backend.surface_width(), backend.surface_height(), TextureFormat::Depth32Float)
    .sample_count(backend.sample_count())
    .usage(TextureUsages::RENDER_ATTACHMENT)
    .build(&backend);
```

A post-process/UI material rendering into the already-resolved surface (or any other non-MSAA target) just leaves `sample_count` at its default of `1`. The same rule applies to [render bundles](./render-bundles.md) — their `sample_count` must match whatever pass they're executed in.
