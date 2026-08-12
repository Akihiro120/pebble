# Observers

Observers are pebble's other communication primitive, alongside [Events](./events.md) — subscription-based instead of polled, and dispatched the same tick they're triggered, not next tick.

Register one on the app:

```rust,ignore
struct Ping(u32);

fn on_ping(trigger: Trigger<Ping>, mut score: Write<Score>) {
    score.0 += trigger.0;
}

app.add_observer(on_ping)
```

An observer function's first parameter is always `Trigger<E>` — it `Deref`s straight to `E`, so `trigger.0`/`trigger.some_field` reads through directly. Every other parameter is an ordinary `SystemParam`, same as a ordinary system.

Fire one via `Commands::trigger`:

```rust,ignore
fn fire(mut commands: Commands) {
    commands.trigger(Ping(3));
}
```

Triggers are queued the same way entity spawns are, and dispatched at the same point in the tick: once the current stage finishes running and commands sync — so an observer registered for `Ping` runs before the end of the stage that triggered it, not one tick later. Multiple observers can be registered for the same event type; all of them run.

## Events vs. Observers

Use **Events** when several systems might poll for something whenever convenient, and it's fine if a couple ticks pass before someone reads it. Use **Observers** when you need a guaranteed, same-tick reaction — the trigger and its handling should feel like one atomic step.
