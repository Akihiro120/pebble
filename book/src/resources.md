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

## A worked example: a resource that isn't ready immediately

Where `Option<Read<T>>` earns its keep is a resource that has to wait on something else *and* shouldn't be rebuilt once it exists. Pebble's own built-in samplers are inserted exactly this way — `init_global_samplers` runs every tick on `AssetSync`, starting well before any `Backend` exists, and does nothing until it does:

```rust,ignore
fn init_global_samplers(
    backend: Option<Read<Backend>>,
    existing: Option<Read<GlobalSamplers>>,
    mut commands: Commands,
) {
    if existing.is_some() {
        return; // already built — don't redo it every tick forever
    }
    let Some(backend) = backend else {
        return; // no backend yet — this system just no-ops and tries again next tick
    };
    let samplers = /* build every SamplerKind against backend */;
    commands.insert_resource(GlobalSamplers { samplers });
}
```

Two `Option` guards doing two different jobs: `backend` gates *when* the work can happen at all, `existing` gates against doing it more than once. This is the same "return `None`, retry next tick" shape the whole [asset pipeline](./the-asset-pipeline.md#why-uploads-retry-instead-of-failing) is built on — resources that depend on other resources just apply it by hand instead of through `Asset::upload`. Reach for this shape for any resource of your own with the same "needs something else first, and is expensive enough to only want built once" profile.
