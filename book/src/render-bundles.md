# Render Bundles

[`RenderBundleEncoder`](../src/wgpu/render_bundle.rs) records a reusable sequence of draw calls once, replayed via `RenderPass::execute_bundles` — worth it once you have many draws that don't change pipeline/bind group/buffers from one frame to the next (static scene geometry, say), since replaying a bundle is typically cheaper than re-recording the same commands by hand every frame.

```rust
let mut encoder = RenderBundleEncoderBuilder::new()
    .color_formats(vec![Some(backend.surface_format())])
    .depth_stencil_format(TextureFormat::Depth32Float) // omit if this pass has no depth attachment
    .sample_count(backend.sample_count())              // must match the pass(es) it's executed in — see MSAA
    .build(&backend);
encoder.set_pipeline(&material.pipeline);
encoder.set_bind_group(0, &instance.bind_group, &[]);
encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
encoder.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
encoder.draw_indexed(0..mesh.index_count, 0, 0..1);
let bundle = encoder.finish(Some("static-geometry"));

// ... later, once per frame, inside an ordinary render pass:
pass.execute_bundles(&[&bundle]);
```

`color_formats`/`depth_stencil_format`/`sample_count` must match whatever render pass(es) the bundle is later executed against, or wgpu's validation rejects it — the same requirement as a material's own `targets`/`depth`/`sample_count` (see [Materials](./materials.md#building-a-material) and [MSAA](./msaa.md)). The recording methods (`set_pipeline`/`set_bind_group`/`set_vertex_buffer`/`set_index_buffer`/`draw`/`draw_indexed`) are the same shape as [`RenderPass`](./rendering-pass-recording.md) itself — a bundle is a record-once, replay-many version of the same vocabulary.
