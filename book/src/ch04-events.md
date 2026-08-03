# Events

Resources are good for "the current state of X." Events are for "something happened" — damage was dealt, a file finished loading, a button was clicked. `Events<T>` is a double-buffered queue: an event sent during tick `N` stays visible to every reader for the rest of `N` and all of `N + 1`, then is dropped.

## Sending and reading

```rust
struct Damage(u32);

app.add_event::<Damage>();

fn deal_damage(mut writer: EventWriter<Damage>) {
    writer.send(Damage(5));
}

fn on_damage(mut reader: EventReader<Damage>) {
    for event in reader.iter() {
        println!("took {} damage", event.0);
    }
}
```

`app.add_event::<T>()` does two things: inserts the `Events<T>` resource, and registers the per-tick aging step that gives the two-tick guarantee above. An `EventWriter<T>`/`EventReader<T>` used before this call panics with a hint pointing back at `add_event` — same hard-requirement mechanism as `Res<T>` from the previous chapter, just checking for `Events<T>` specifically.

## Why two ticks?

Systems run in a fixed order within a tick, so a reader registered *before* the writer in the stage order would never see a same-tick send if events only lived for the tick they were sent in — it already ran by the time the writer fires. Keeping an event visible through the *next* tick as well means every reader sees every event exactly once, regardless of where in the pipeline it happens to run relative to the writer. Each `EventReader<T>` tracks its own read cursor privately (the same way `Local<T>` persists per-system state), so multiple independent readers of the same event type never interfere with each other.

## `Option<EventReader<T>>` / `Option<EventWriter<T>>`

Exactly like `Option<Res<T>>`: use these when a system should just skip its event-related work if `T` hasn't been registered yet, instead of hard-panicking:

```rust
fn maybe_log_damage(reader: Option<EventReader<Damage>>) {
    let Some(mut reader) = reader else { return };
    for event in reader.iter() {
        // ...
    }
}
```

This matters most for library-ish code — a plugin that optionally reacts to an event type the *host* application may or may not have registered, without forcing that application to always register it.

Events whose payload arrives from a background task — a downloaded file, a GPU readback — use a different constructor, `add_async_event`, covered in the next chapter alongside the rest of Pebble's async story.
