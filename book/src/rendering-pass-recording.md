# Recording a Render Pass

## Starting a pass

`render_context(clear_color)` (a shortcut for one color attachment, no depth) or `begin_pass(Pass { .. })` (the general form) hand back a [`RenderPass`](../src/wgpu/render_pass.rs) — opaque like everything else in `pebble::wgpu`: no raw `wgpu::RenderPass` anywhere, draw-time operations are methods instead:

```rust
let mut pass = active.render_context([0.05, 0.05, 0.08, 1.0]);
pass.set_pipeline(&material.pipeline);       // &RenderPipeline
pass.set_bind_group(0, &instance.bind_group, &[]); // &BindGroup, dynamic offsets last
pass.set_vertex_buffer(0, &mesh.vertex_buffer);    // &Buffer, whole buffer
pass.set_index_buffer(&mesh.index_buffer, IndexFormat::Uint32); // &Buffer + IndexFormat
pass.draw_indexed(0..mesh.index_count, 0, 0..1);
pass.draw(0..vertex_count, 0..1);            // non-indexed
```

`IndexFormat` (`Uint16`/`Uint32`) mirrors `wgpu::IndexFormat` — the same two variants, just not the raw type. See [Materials](./materials.md#rendering-with-a-material-and-instance) for the full setup this snippet assumes (a material, an instance, a mesh, all already uploaded).

## A custom render target or depth attachment

`begin_pass` takes a `Pass { colors: &[ColorTarget], depth: Option<DepthTarget> }` — `render_context` is just a convenience wrapper over it for the single-color-no-depth case:

```rust
let mut pass = active.begin_pass(Pass {
    colors: &[ColorTarget::default([0.2, 0.3, 0.3, 1.0])],
    depth: Some(DepthTarget::new(&depth_view, 1.0)),
});
```

`ColorTarget::Custom { attachment, clear }` renders into an opaque `TextureView` (see [Textures](./textures.md#a-render-target--depth-buffer-no-source-data)) instead of the window surface — an offscreen pass, a cubemap face capture, a shadow map. `DepthTarget::new(view, 1.0)` clears the depth buffer to the far plane (`1.0`) at the start of the pass — a fragment only writes if its depth compares `Less` than what's already there, so nearer geometry wins regardless of draw order. See [Custom GPU Resources](./custom-gpu-resources.md) for a full depth-tested, camera-bound example.

## Indirect draws

`draw_indirect`/`draw_indexed_indirect` read the vertex/instance counts from a buffer instead of a CPU-known value — for a draw count the GPU itself computed (culling compaction, LOD selection, ...):

```rust
let args = DrawIndirectArgs { vertex_count: 3, instance_count: 1, first_vertex: 0, first_instance: 0 };
let indirect_buffer = BufferBuilder::with_data(args.as_bytes()).usage(BufferUsages::INDIRECT | BufferUsages::COPY_DST).build(&backend);

pass.draw_indirect(&indirect_buffer, 0);          // reads a DrawIndirectArgs at byte offset 0
pass.draw_indexed_indirect(&indirect_buffer, 0);  // reads a DrawIndexedIndirectArgs instead
```

`DrawIndirectArgs`/`DrawIndexedIndirectArgs` mirror `wgpu`'s own argument layouts field-for-field — typically written by a compute shader into a storage buffer (also usable as `INDIRECT`) rather than built on the CPU like the example above. The compute-side equivalent is covered in [Compute Pipelines](./compute-pipelines.md#indirect-dispatch).
