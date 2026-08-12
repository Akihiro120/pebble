# Systems and Stages

A system is a plain function whose parameters are all `SystemParam`s — `Read<T>`/`Write<T>`, `Query<Q>`, `Local<T>`, `Commands`, `EventReader<T>`/`EventWriter<T>`, and tuples of these. Pebble infers everything from the function signature; there's no registration macro or trait to implement:

```rust,ignore
fn my_system(time: Read<Time>, mut player: Query<(&mut Position, &Velocity)>) {
    for (pos, vel) in player.iter() {
        pos.0 += vel.0 * time.delta_seconds();
    }
}
```

Register it on a stage:

```rust,ignore
app.add_system(SystemStage::Update, my_system)
```

## Stages

Stages run in this order, every tick:

| Stage | Runs |
|---|---|
| `Startup` | Once, before anything else — no GPU backend yet, pure CPU setup. |
| `Ready` | Once, automatically, the first tick the GPU backend is ready. For one-time setup that needs `Backend`/`Assets<T>` — building your first mesh, material, etc. |
| `AssetSync` | Uploads CPU-side assets to the GPU, retrying until dependencies are met. |
| `PreUpdate` | Before main game logic — input, timers, event aging. |
| `Update` | Main game logic. |
| `PostUpdate` | After main game logic. |
| `PreRender` | Acquires the frame. |
| `Render` | Issue draw calls. |
| `PostRender` | Submit and present. |

`Startup` and `Ready` each run exactly once and are then removed from their app's schedule — registering a second system on either later just adds to what runs that one time, it doesn't create a second occurrence.

## Why `Ready` instead of `Startup` for GPU setup

`Startup` runs before the GPU backend has had a single tick to be acquired, so `Read<Backend>` there always panics. `Ready` runs the moment the backend actually exists — a plain `Read<Backend>` there is always safe, no `Option` guard needed:

```rust,ignore
fn setup(backend: Read<Backend>, mut meshes: Write<Assets<Mesh>>) {
    Mesh::new(vertices, indices).build_asset("player", &mut meshes);
}

app.add_system(SystemStage::Ready, setup)
```

## Local state

`Local<T>` gives a system its own private, per-system `T` that persists between ticks — a frame counter, a "have I already fired" flag. Two different systems, even with the same `T`, never share one:

```rust,ignore
fn count_ticks(mut ticks: Local<u32>) {
    *ticks += 1;
}
```
