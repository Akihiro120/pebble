# Queries, Commands, and Entities

Pebble's ECS world is [`hecs`](https://docs.rs/hecs) underneath, but no `hecs` type appears in `Query`'s or `Commands`'s own public API — you write plain Rust tuples of components.

## Queries

`Query<Q>` fetches components matching `Q` from every entity that has them:

```rust,ignore
fn move_things(mut q: Query<(&mut Position, &Velocity)>, time: Read<Time>) {
    for (pos, vel) in q.iter() {
        pos.0 += vel.0 * time.delta_seconds();
    }
}
```

Other methods:

- `get(entity)` — fetch one known entity directly, `None` if it doesn't exist or doesn't match `Q`.
- `with::<R>()`/`without::<R>()` — narrow to entities that also have (or don't have) component(s) `R`, without `R` joining the yielded items. Chainable.
- `single()`/`get_single()` — expect exactly one matching entity (the player, the active camera). `single` panics if that's not true; `get_single` returns `None` instead.

## Commands

`Commands` queues entity spawns/despawns and resource mutations, applied once the current stage finishes running — not immediately. It `Deref`s to `hecs::CommandBuffer` for entity operations:

```rust,ignore
fn spawn_enemy(mut commands: Commands) {
    commands.spawn((Position(Vec2::ZERO), Enemy));
}

fn despawn_dead(mut commands: Commands, q: Query<(&Health,)>) {
    for (entity, (health,)) in q.iter() {
        if health.0 <= 0 {
            commands.despawn(entity);
        }
    }
}
```

It also has its own methods, independent of `hecs::CommandBuffer`:

- `insert_resource(value)`/`remove_resource::<T>()` — deferred resource mutation, same effect as `App::insert_resource` but callable from inside a system.
- `trigger(event)` — see [Observers](./observers.md).

Deferring to end-of-stage means a spawn queued by one system in `Update` is visible to a `Query` in `PostUpdate` that same tick, but not to another `Update` system that runs after it in the same stage.
