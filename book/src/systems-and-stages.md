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

Built-in plugins that own a `Ready` system (`GraphicsPlugin`'s `init_global_samplers`, for instance) register it with `.priority(ENGINE_READY_PRIORITY)` — a constant equal to `i32::MAX`. That guarantees engine setup always wins the tie-break against your own `Ready` systems, which default to priority `0`, so engine resources it builds (`GlobalSamplers`, etc.) are already present for any user `Ready`-stage system that reads them the same tick — regardless of add-order. Give your own plugin's `Ready` systems this same priority if other users' code (or your own later setup) needs to depend on them without an explicit `.after(...)`.

## System ordering

Systems on the same stage normally run in the order they were added. Three tools change that, and can be combined:

- **`.after(...)`/`.before(...)`** — pin a system relative to another one known to the stage. The other system doesn't need to be added yet; only its type is used, resolved when the schedule's order is next computed.
- **`.priority(n: i32)`** — break ties between systems that have no `after`/`before` relationship to each other; higher runs first. Defaults to `0`. An explicit `after`/`before` constraint always wins over priority — priority only decides among systems the schedule would otherwise be free to run in any order.
- **`.chain()`** — called on a tuple of systems, e.g. `(a, b, c).chain()`, forces them to run in exactly that relative order. Register the result with `add_systems` (plural), not `add_system`. A chain can itself take `.after(...)`/`.before(...)`/`.priority(...)`, applied to the whole chain — `.after`/`.before` only need to constrain the chain's first/last system respectively, since the rest already transitively depend on it; `.priority` applies to every member, since each competes for its own slot as it individually becomes eligible to run.

```rust,ignore
app.add_system(SystemStage::Update, spawn_enemies)
    .add_system(SystemStage::Update, move_enemies.after(spawn_enemies))
    .add_system(SystemStage::Update, render.after(move_enemies))
    .add_system(SystemStage::Update, hud.priority(10))
    .add_systems(
        SystemStage::Update,
        (physics_step, resolve_collisions).chain().before(render),
    );
```

## Local state

`Local<T>` gives a system its own private, per-system `T` that persists between ticks — a frame counter, a "have I already fired" flag. Two different systems, even with the same `T`, never share one:

```rust,ignore
fn count_ticks(mut ticks: Local<u32>) {
    *ticks += 1;
}
```
