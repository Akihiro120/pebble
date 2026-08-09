# Systems and Stages

## Systems are plain functions

A system is any function whose parameters are all `SystemParam`s. Pebble inspects the signature, fetches each parameter, and calls the function — no registration macro, no trait to implement by hand:

```rust
fn move_system(
    time: Res<Time>,              // immutable resource borrow
    mut rb: ResMut<RigidBodies>,  // mutable resource borrow
    mut q: Query<&mut Transform>, // ECS query
    mut cmd: Commands,            // deferred world mutations
) {
    // ...
}

app.add_system(SystemStage::Update, move_system);
```

`Query` and `Commands` are covered in [Queries, Commands, and Entities](./queries-commands-entities.md); `Res`/`ResMut` in [Resources](./resources.md). This page is about the other axis: when a system runs at all.

## Stages: when a system runs

Every system is registered against a `SystemStage`, which determines its place in the fixed per-tick order:

| Stage | Purpose |
|---|---|
| `Startup` | One-time initialization before the first tick (resources, GPU objects) |
| `PreUpdate` | Before main logic (input, time, draining channels) |
| `Update` | Main game logic |
| `PostUpdate` | After main logic |
| `PreRender` | Prepare render data, poll the backend |
| `AssetSync` | Upload CPU assets to the GPU backend |
| `AssetSyncDeps` | Upload assets that depend on other GPU assets |
| `Render` | Issue draw calls |
| `PostRender` | Present the frame |

`AssetSync`/`AssetSyncDeps` are special: they run to convergence (repeated until a full pass produces nothing new) at the very front of every tick, and again after every other stage — so newly queued asset work is drained immediately instead of waiting for next tick's front pass. See [The Asset Pipeline and Handles](./the-asset-pipeline.md#why-two-assetsync-stages) for why they're split into two.

## Registering several systems at once

```rust
app.add_systems(SystemStage::Update, (system_a, system_b, system_c));
```

## Ordering within a stage

Systems in the same stage otherwise run in registration order — `.before()`/`.after()` impose an explicit constraint instead:

```rust
app.add_systems(SystemStage::Update, (
    apply_input,
    move_player.after(apply_input),
));
```

## Run once

A function that returns `Option<()>` is automatically a once-system — no wrapper needed:

```rust
fn spawn_scene(mut commands: Commands, config: Option<Res<MyConfig>>) -> Option<()> {
    let config = config?; // not ready yet — try again next tick
    commands.spawn(/* ... */);
    Some(()) // done — never runs again
}

app.add_system(SystemStage::PreUpdate, spawn_scene);
```

Return `None` to mean "call me again next tick"; return `Some(())` to mean "done" — the system is retired permanently, no matter how many ticks that took. It composes with the hard-requirement check described in [Resources](./resources.md#what-happens-when-a-resource-isnt-there-yet): a bare `Res<T>` parameter is still checked (wait if declared, panic if not) before the function body ever runs.

`SystemStage::Startup` is the natural home for one-time initialization — resources that need the GPU device, initial entity spawning, etc. Like any other stage, a `Startup` system that returns `Option<()>` retries every startup pass until it succeeds. One-off GPU resources follow exactly this pattern — see [Custom GPU Resources](./custom-gpu-resources.md) for the full walkthrough.

## Run conditions

`.run_if::<C>()` gates a system (or a whole tuple passed to `add_systems`) behind a `RunCondition`, re-checked every tick — its `SystemParam`s are only fetched, and its body only runs, when the condition holds:

```rust
app.add_systems(
    SystemStage::Update,
    expensive_diagnostic.run_if::<ResourceExists<DebugOverlay>>(),
);
```

Built-in conditions: `ResourceExists<T>`, plus `And<A, B>`/`Or<A, B>` for combining conditions; implement `RunCondition` yourself for anything else. A system wrapped in `.run_if` is fully exempt from the hard-requirement panic described in [Resources](./resources.md#what-happens-when-a-resource-isnt-there-yet) — the condition is trusted to gate correctly, so a bare `Res<T>` inside it is never checked independently.
