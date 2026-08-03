# Systems, Stages, and Resources

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

`Query` and `Commands` are covered in the next chapter. This chapter is about the other two: resources, and when a system runs at all.

## Stages: when a system runs

Every system is registered against a `SystemStage`, which determines its place in the fixed per-tick order:

| Stage | Purpose |
|---|---|
| `PreUpdate` | Before main logic (input, time, draining channels) |
| `Update` | Main game logic |
| `PostUpdate` | After main logic |
| `PreRender` | Prepare render data, poll the backend |
| `AssetSync` | Upload CPU assets to the GPU backend |
| `AssetSyncDeps` | Upload assets that depend on other GPU assets |
| `Render` | Issue draw calls |
| `PostRender` | Present the frame |

`AssetSync`/`AssetSyncDeps` are special: they run to convergence (repeated until a full pass produces nothing new) at the very front of every tick, and again after every other stage — so newly queued asset work is drained immediately instead of waiting for next tick's front pass. [The Asset Pipeline and Handles](./ch06-assets-and-handles.md) covers why they're split into two.

Within one stage, systems run in the order they were registered, unless you impose an explicit ordering constraint — not covered in this book; see the `System` trait's docs for `before`/`after`.

## Resources: singleton state

A resource is any `hecs::Component` type with exactly one instance, stored in the ECS world rather than on an entity:

```rust
app.add_resource(MyConfig { volume: 0.8 });

fn my_system(config: Res<MyConfig>) {
    println!("{}", config.volume);
}
```

`Res<T>`/`ResMut<T>` borrow it immutably/mutably for the duration of the system call — the same borrow-checking rules as `RefCell` apply across the whole tick, so two systems in the same stage both wanting `ResMut<T>` is fine (they run sequentially), but you can't stash a `Res<T>` guard somewhere and read it later.

## What happens when a resource isn't there yet

A bare `Res<T>`/`ResMut<T>` is a **hard requirement**: before a system with one runs, Pebble checks that `T` actually exists. What happens if it doesn't depends on whether anything has *declared* it will eventually provide `T`:

- **Something declared it** (a `LazyResource` plugin, an async graphics backend) — the system is silently skipped this pass and retried next tick. No error; this is the expected shape of "constructed asynchronously."
- **Nothing declared it** — `App` panics immediately, naming both the system and the missing resource, with a hint pointing at the fix (usually a missing `app.add_resource(...)` or a missing plugin).

This is why `build()` runs its own pre-flight pass (see [previous chapter](./ch01-app-and-plugins.md)): it applies exactly this check to every system in every stage before `run()` starts, so a missing-resource mistake becomes one clear panic at startup instead of a surprise several ticks in.

When a resource is *legitimately* optional — not "not ready yet," but "may never exist, and that's fine" — use `Option<Res<T>>` instead. It never panics or waits; the system just receives `None` and can skip its own work:

```rust
fn maybe_render(backend: Option<Res<WGPUBackend>>) {
    let Some(backend) = backend else { return }; // backend not ready yet, try again next tick
    // ...
}
```

## Run once

There's no dedicated "Startup" stage (see the end of the previous chapter) — instead, `.once()` turns "have I already done this" into the system's own return value:

```rust
fn spawn_scene(mut commands: Commands, config: Option<Res<MyConfig>>) -> Option<()> {
    let config = config?; // not ready yet — try again next tick
    commands.spawn(/* ... */);
    Some(()) // done — never runs again
}

app.add_system(SystemStage::PreUpdate, spawn_scene.once());
```

Return `None` to mean "call me again next tick"; return `Some(())` to mean "done" — the system is retired permanently, no matter how many ticks that took. It composes with the hard-requirement check above: a bare `Res<T>` parameter inside a `.once()` system is still checked (wait if declared, panic if not) before the function body ever runs.

## Run conditions

`.run_if::<C>()` gates a system (or a whole tuple passed to `add_systems`) behind a `RunCondition`, re-checked every tick — its `SystemParam`s are only fetched, and its body only runs, when the condition holds:

```rust
app.add_systems(
    SystemStage::Update,
    expensive_diagnostic.run_if::<ResourceExists<DebugOverlay>>(),
);
```

Built-in conditions: `ResourceExists<T>`, plus `And<A, B>`/`Or<A, B>` for combining conditions; implement `RunCondition` yourself for anything else. A system wrapped in `.run_if` is fully exempt from the hard-requirement panic described above — the condition is trusted to gate correctly, so a bare `Res<T>` inside it is never checked independently.
