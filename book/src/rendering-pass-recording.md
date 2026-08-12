# Recording a Render Pass

`BackendPlugin` (part of `GraphicsPlugin`) acquires the swapchain frame on `SystemStage::PreRender` and submits/presents it on `PostRender`. Your own drawing goes on `SystemStage::Render`, in between.

```rust,ignore
fn draw(mut frame: Write<CurrentFrame>) {
    let Some(mut active) = frame.active() else { return }; // no frame this tick — skip

    let pass = PassBuilder::new()
        .with_target(ColorTargetBuilder::new().with_clear([0.1, 0.1, 0.1, 1.0]).build())
        .build();

    let mut render_pass = active.begin_pass(pass);
    render_pass.set_pipeline(&material.pipeline);
    render_pass.set_bind_group(0, &instance.bind_group, &[]);
    render_pass.set_vertex_buffer(0, &mesh.vertex_buffer);
    render_pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32);
    render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
}
```

`CurrentFrame::active()` returns `None` when the surface couldn't be acquired this tick (occluded, mid-resize, etc.) — always check it and skip rendering rather than unwrapping.

## Targets

`ColorTargetBuilder`/`DepthTargetBuilder`/`PassBuilder` describe what to render into:

- An unattached color target (`ColorTargetBuilder::new()` with no `.with_attachment(...)`) falls back to the swapchain's own view — the common case for drawing directly to the screen.
- `.with_attachment(&texture_view)` points a color target at your own texture instead, e.g. for post-processing — see [Textures](./textures.md) for building a `TextureView` render target.
- A depth target always points at your own texture; there's no swapchain depth buffer. Zero color targets plus a depth target is a valid, depth-only pass (a shadow map).

## During the pass

`RenderPass` mirrors `wgpu`'s own API closely: `set_pipeline`, `set_bind_group`, `set_vertex_buffer`, `set_index_buffer`, `draw`, `draw_indexed`, and their indirect variants `draw_indirect`/`draw_indexed_indirect`. For the indirect draws, `DrawIndirectArgs`/`DrawIndexedIndirectArgs` give the exact byte layout the GPU expects — write one via `bytemuck::bytes_of` (or its own `.as_bytes()`) into a buffer built with `BufferUsages::INDIRECT`.
