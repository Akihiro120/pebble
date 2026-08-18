# Helper Macros

A small set of `#[macro_export]` macros that collapse the boilerplate that shows up constantly around asset lookups and drawing. They're plain `macro_rules!`, exported at the crate root — `pebble::or_return!`, `pebble::bind_mat!`, etc. — no prelude needed.

## `or_return!`

Systems return `()`, so the `?` operator isn't available the way it would be in a function returning `Option`/`Result`. `or_return!` collapses the `let Some(x) = expr else { return };` pattern that fills the gap:

```rust,ignore
fn draw(materials: Read<Assets<Material>>) {
    let material = or_return!(materials.get(handle));
    // ...
}
```

Pass a second argument to return something other than `()`:

```rust,ignore
let material = or_return!(materials.get(handle), return None);
```

The other three macros are built on top of `or_return!`, so they inherit this same early-return-when-not-ready behavior — an asset that hasn't finished uploading yet is a normal, common case (a couple frames on load), not a bug to `unwrap()` past.

## `bind_mat!` / `bind_comp!`

The lookup-then-bind every draw call repeats: `materials.get(handle)`, then `set_pipeline` + `set_bind_group(0, ...)`:

```rust,ignore
fn draw(
    mut frame: Write<CurrentFrame>,
    materials: Read<Assets<Material>>,
) {
    let Some(mut active) = frame.active() else { return };
    let pass = PassBuilder::new().build();
    let mut render_pass = active.begin_pass(pass);

    bind_mat!(render_pass, materials, material_handle);
}
```

It evaluates to the looked-up `&GPUMaterial`, so grab it with `let` if you also need `.update(name, data)`/`.update_value(name, &value)` on it (e.g. a per-frame camera uniform):

```rust,ignore
let material = bind_mat!(render_pass, materials, material_handle);
material.update("camera", bytemuck::bytes_of(&camera_data));
```

`bind_comp!` is the same thing for a `ComputePass` + [`Compute`](./compute-pipelines.md) instead of a `RenderPass` + [`Material`](./materials.md).

There's no combined "bind and draw" macro — a draw call sometimes needs more than one bind group (material at group 0, something else at group 1) before drawing, so `bind_mat!`/`bind_comp!` stay composable rather than folded into one rigid macro.

## `draw_mesh!`

Sets the vertex/index buffers and calls `draw_indexed` for a mesh handle, defaulting the instance range to `0..1`:

```rust,ignore
draw_mesh!(render_pass, meshes, mesh_handle);
draw_mesh!(render_pass, meshes, mesh_handle, 0..enemy_count); // instanced draw
```

## Putting it together

The [Recording a Render Pass](./rendering-pass-recording.md) example, using the full suite:

```rust,ignore
fn draw(
    mut frame: Write<CurrentFrame>,
    materials: Read<Assets<Material>>,
    meshes: Read<Assets<Mesh>>,
) {
    let Some(mut active) = frame.active() else { return }; // no frame this tick — skip

    let pass = PassBuilder::new()
        .with_target(ColorTargetBuilder::new().with_clear([0.1, 0.1, 0.1, 1.0]).build())
        .build();

    let mut render_pass = active.begin_pass(pass);
    bind_mat!(render_pass, materials, material_handle);
    draw_mesh!(render_pass, meshes, mesh_handle);
}
```

Five lines of lookups and wiring down to two, with the same not-ready-yet handling as the manual version.
