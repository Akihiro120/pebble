# Resources

A resource is a singleton value keyed by type — one `Time`, one `Assets<Mesh>`, one of whatever app-wide state you define. Insert one from the `App` builder:

```rust,ignore
app.insert_resource(Score(0))
```

or from within a system, deferred, via [`Commands`](./queries-commands-entities.md).

## Reading and writing

Take `Read<T>` or `Write<T>` as a system parameter:

```rust,ignore
fn print_score(score: Read<Score>) {
    println!("{}", score.0);
}

fn add_point(mut score: Write<Score>) {
    score.0 += 1;
}
```

Both borrow-check at runtime, not compile time — two systems that both take `Write<T>` for the same `T` will panic if they somehow ran concurrently, but pebble's scheduler runs systems one at a time within a stage, so in practice this only bites if you hold a borrow across an await point or similar, which the API doesn't allow you to do anyway.

`Read`/`Write` panic at fetch time if `T` isn't present. For a resource that might not exist yet — a GPU backend before it's acquired, a plugin that's optional — take `Option<Read<T>>`/`Option<Write<T>>` instead:

```rust,ignore
fn maybe_render(backend: Option<Read<Backend>>) {
    let Some(backend) = backend else { return };
    // ...
}
```

In practice you rarely need the `Option` form for `Backend` specifically — see [`SystemStage::Ready`](./systems-and-stages.md#why-ready-instead-of-startup-for-gpu-setup), which exists precisely so ordinary `Read<Backend>` is safe from `Ready` onward.

## A worked example: a resource that only needs `Backend`

If a resource of your own needs nothing but `Backend` to build — no other resource, no per-tick retry — don't reach for `Option<Read<Backend>>` at all. Register the system on `SystemStage::Ready` instead: it runs exactly once, automatically, the first tick `Backend` exists, so a plain `Read<Backend>` inside it is always safe, and there's no "already built?" guard to write because it physically can't run twice.

Pebble's own built-in samplers work this way — `init_global_samplers` is registered on `Ready`:

```rust,ignore
fn init_global_samplers(backend: Read<Backend>, mut commands: Commands) {
    let samplers = /* build every SamplerKind against backend */;
    commands.insert_resource(GlobalSamplers { samplers });
}
```

It's registered with `.priority(ENGINE_READY_PRIORITY)` (see [Systems and Stages](./systems-and-stages.md#why-ready-instead-of-startup-for-gpu-setup)), so it always runs before other `Ready`-stage systems regardless of add-order. `GlobalSamplers` ends up with the exact same guarantee `Backend` itself has — present from `Ready` onward, so downstream systems (any other `Ready`-stage system, and anything in `AssetSync`/`PreUpdate`/`Update`/... since those run later in the tick) can just take `Read<GlobalSamplers>`, no `Option` needed.

## When you actually need the `Option`-guard-and-retry shape

Reach for `Option<Read<T>>` when a resource depends on something that *isn't* guaranteed by a fixed one-shot stage — most commonly, another resource that's itself built asynchronously (an uploaded asset, a resource inserted from `AssetSync`), or state that can legitimately not exist yet for reasons `Ready` doesn't capture:

```rust,ignore
fn maybe_render(backend: Option<Read<Backend>>) {
    let Some(backend) = backend else { return };
    // ...
}
```

For a resource that needs to wait on something else *and* shouldn't be rebuilt once it exists, use two guards doing two different jobs — one gating *when* the work can happen, one gating against doing it more than once:

```rust,ignore
fn init_thing(dep: Option<Read<SomeAsyncResource>>, existing: Option<Read<Thing>>, mut commands: Commands) {
    if existing.is_some() {
        return; // already built — don't redo it every tick forever
    }
    let Some(dep) = dep else {
        return; // dependency not ready yet — no-op and try again next tick
    };
    commands.insert_resource(Thing::from(&*dep));
}
```

This is the same "return `None`, retry next tick" shape the whole [asset pipeline](./the-asset-pipeline.md#why-uploads-retry-instead-of-failing) is built on — resources that depend on other resources just apply it by hand instead of through `Asset::upload`. Reach for it only when your dependency isn't itself covered by a one-shot stage like `Ready`.
