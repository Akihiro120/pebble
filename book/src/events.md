# Events

Events are for many-to-many, poll-when-convenient communication — damage dealt, an item picked up, anything a handful of unrelated systems might want to react to without tight coupling. If you instead want guaranteed same-tick reactions, see [Observers](./observers.md).

Register the event type on the app:

```rust,ignore
app.add_event::<Damage>()
```

This is idempotent — safe to call from two different plugins that both want `Damage`.

## Sending and reading

```rust,ignore
struct Damage(u32);

fn deal_damage(mut writer: EventWriter<Damage>) {
    writer.send(Damage(10));
}

fn log_damage(mut reader: EventReader<Damage>) {
    for event in reader.iter() {
        println!("took {} damage", event.0);
    }
}
```

`Events<T>` is double-buffered: an event sent during tick `N` is visible to readers for the rest of `N` and all of `N + 1`, then dropped. Each `EventReader` keeps its own read cursor (like `Local`), so a reader sees every event exactly once no matter when it runs relative to the writer, and multiple readers of the same type don't interfere with each other.

For a resource/event type that might not be registered — e.g. an optional plugin's event — take `Option<EventReader<T>>`/`Option<EventWriter<T>>` instead.

## Signaling that an async result is ready

A particularly useful case: a system polling a [`Promise<T>`](./promise.md) (a GPU readback, say) and another system that needs to react once the result lands. Sending an event when the `Promise` resolves is usually a better fit than publishing the result as a resource — see [Promise: handing the result to a dependent system via an Event](./promise.md#walkthrough-3-handing-the-result-to-a-dependent-system-via-an-event) for the full walkthrough, including why `send` beats `Commands::insert_resource` for same-tick delivery.
