# Time

`TimePlugin` inserts a `Time` resource and ticks it every frame on `SystemStage::PreUpdate`:

```rust,ignore
app.add_plugin(TimePlugin)
```

```rust,ignore
fn move_player(time: Read<Time>, mut pos: Write<Position>) {
    pos.0 += velocity * time.delta_seconds();
}
```

- `delta()`/`delta_seconds()` — time since the previous tick.
- `elapsed()`/`elapsed_seconds()` — total time since the app started.
- `fps()` — `1.0 / delta_seconds()`, or `0.0` on the very first tick.

`GraphicsPlugin` does not add `TimePlugin` for you — add it explicitly if you need `Time`.
