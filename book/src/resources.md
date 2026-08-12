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
