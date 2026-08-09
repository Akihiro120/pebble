# Resources

A resource is any `hecs::Component` type with exactly one instance, stored in the ECS world rather than on an entity — the place for singleton, shared state (config, the current camera, a GPU backend).

## Reading and writing — `Res<T>` / `ResMut<T>`

```rust
app.add_resource(MyConfig { volume: 0.8 });

fn my_system(config: Res<MyConfig>) {
    println!("{}", config.volume);
}

fn my_other_system(mut config: ResMut<MyConfig>) {
    config.volume = 1.0;
}
```

`Res<T>`/`ResMut<T>` borrow it immutably/mutably for the duration of the system call — the same borrow-checking rules as `RefCell` apply across the whole tick, so two systems in the same stage both wanting `ResMut<T>` is fine (they run sequentially), but you can't stash a `Res<T>` guard somewhere and read it later.

## What happens when a resource isn't there yet

A bare `Res<T>`/`ResMut<T>` is a **hard requirement**: before a system with one runs, Pebble checks that `T` actually exists. What happens if it doesn't depends on whether anything has *declared* it will eventually provide `T`:

- **Something declared it** (a startup system, an async graphics backend) — the system is silently skipped this pass and retried next tick. No error; this is the expected shape of "constructed asynchronously."
- **Nothing declared it** — `App` panics immediately, naming both the system and the missing resource, with a hint pointing at the fix (usually a missing `app.add_resource(...)` or a missing plugin).

This is why `build()` runs its own pre-flight pass (see [Apps and Plugins](./apps-and-plugins.md#what-build-actually-does)): it applies exactly this check to every system in every stage before `run()` starts, so a missing-resource mistake becomes one clear panic at startup instead of a surprise several ticks in.

## A genuinely optional resource — `Option<Res<T>>`

When a resource is *legitimately* optional — not "not ready yet," but "may never exist, and that's fine" — use `Option<Res<T>>`/`Option<ResMut<T>>` instead. It never panics or waits; the system just receives `None` and can skip its own work:

```rust
fn maybe_render(backend: Option<Res<WGPUBackend>>) {
    let Some(backend) = backend else { return }; // backend not ready yet, try again next tick
    // ...
}
```

## Per-system local state — `Local<T>`

Not shared through `Resources` at all — each system gets its own private `T: Default`, persisted across every subsequent run of that specific system, invisible to every other system:

```rust
fn count_ticks(mut count: Local<u32>) {
    *count += 1;
}
```

Useful for a system's own bookkeeping (a debounce timer, a "have I logged this already" flag) that has no business being a shared resource.
