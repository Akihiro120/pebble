# Queries, Commands, and Entities

Pebble's ECS world is [`hecs`](https://docs.rs/hecs) underneath. `Query`/`Commands` themselves stay unopinionated about it — you write plain Rust tuples of components — but entity IDs and `hecs::CommandBuffer` (which `Commands` `Deref`s to) do genuinely surface the moment you need to work with entities directly, as this page's examples show. `Entity` itself is re-exported as `pebble::ecs::Entity`, so you don't need `hecs` as your own dependency just to spell that one type — but a couple of more advanced patterns below (anything touching `&hecs::World` directly) do still need it.

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

### Getting entity IDs out of a query

`q.iter()` yields *only* whatever's in `Q` — a `Query<(&Health,)>` iterates `&Health` values with no way to know which entity each one came from. `Entity` implements `hecs::Query` in its own right, so the fix is to ask for it explicitly, as one more element of the tuple:

```rust,ignore
use pebble::ecs::Entity;

fn find_low_health(mut q: Query<(Entity, &Health)>) {
    for (entity, health) in q.iter() {
        if health.0 < 10 {
            // pass `entity` to commands.despawn(...), store it in a resource,
            // whatever you need
        }
    }
}
```

## Commands

`Commands` queues entity spawns/despawns and resource mutations, applied once the current stage finishes running — not immediately. It `Deref`s to `hecs::CommandBuffer` for entity operations:

```rust,ignore
fn spawn_enemy(mut commands: Commands) {
    commands.spawn((Position(Vec2::ZERO), Enemy));
}
```

It also has its own methods, independent of `hecs::CommandBuffer`:

- `insert_resource(value)`/`remove_resource::<T>()` — deferred resource mutation, same effect as `App::insert_resource` but callable from inside a system.
- `trigger(event)` — see [Observers](./observers.md).

Deferring to end-of-stage means a spawn queued by one system in `Update` is visible to a `Query` in `PostUpdate` that same tick, but not to another `Update` system that runs after it in the same stage.

### Getting the `Entity` back from a deferred spawn

`CommandBuffer::spawn` doesn't return an `Entity` — it can't, since the actual spawn hasn't happened yet, it's just queued. If you need the ID *right away* (to store in a resource, hand to another system this same tick, etc.), reserve one up front with `hecs::World::reserve_entity()` — available via the `&hecs::World` system parameter — then queue an `insert` against that already-known ID instead of a `spawn`:

```rust,ignore
fn spawn_and_track(world: &hecs::World, mut commands: Commands, mut tracker: Write<SpawnedEnemies>) {
    let entity = world.reserve_entity();
    commands.insert(entity, (Position(Vec2::ZERO), Enemy));
    tracker.0.push(entity); // valid to use immediately, even though the World
                             // doesn't actually have this entity until commands sync
}
```

## Putting it together: a full lifecycle

Spawn, react to state each tick, and clean up — the whole loop an enemy goes through, using only what's on this page:

```rust,ignore
fn spawn_wave(world: &hecs::World, mut commands: Commands) {
    for _ in 0..5 {
        let entity = world.reserve_entity();
        commands.insert(entity, (Position(random_spawn_point()), Health(100), Enemy));
    }
}

fn apply_damage(mut q: Query<(&mut Health, &RecentHit)>) {
    for (health, hit) in q.iter() {
        health.0 -= hit.amount;
    }
}

fn despawn_dead(mut q: Query<(Entity, &Health)>, mut commands: Commands) {
    for (entity, health) in q.iter() {
        if health.0 <= 0 {
            commands.despawn(entity);
        }
    }
}
```

Register all three on `SystemStage::Update`, in that order — `apply_damage` runs against components already in the `World` this tick (including anything spawned in a *previous* tick's `Update`, since that spawn's commands already synced), and `despawn_dead` sees the health values `apply_damage` just wrote, since they're plain mutations through `&mut Health`, not deferred through `Commands`. If `despawn_dead` should also notify other systems when an enemy dies — awarding score, playing a sound — see [Observers](./observers.md#why-multiple-observers-on-one-trigger-is-the-actual-point) for the natural next step: `commands.trigger(EnemyDied { .. })` right alongside the `despawn`.
