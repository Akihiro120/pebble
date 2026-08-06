# Queries, Commands, and Entities

## Components and entities

An entity is just an id; components are plain Rust values attached to it. Spawn one with `Commands`:

```rust
fn spawn_entities(mut commands: Commands) -> Option<()> {
    commands.spawn((
        Name("wanderer"),
        Position { x: 0.0, y: 0.0 },
        Velocity { dx: 1.0, dy: 2.0 },
    ));
    Some(())
}

app.add_system(SystemStage::PreUpdate, spawn_entities.once());
```

Any tuple of types works as the component set — no registration step, no marker trait to implement. A component with no data at all (`struct Asleep;`) is a common, useful pattern purely for tagging entities so a query can filter by "has this," regardless of what else it carries.

## Querying

`Query<Q>` fetches every entity matching the component set `Q`. Iterate it with `.iter()`:

```rust
fn move_entities(mut query: Query<(&Name, &mut Position, &Velocity)>) {
    for (name, pos, vel) in query.iter() {
        pos.x += vel.dx;
        pos.y += vel.dy;
        println!("{} moved to ({:.1}, {:.1})", name.0, pos.x, pos.y);
    }
}
```

Mixing `&T` (read) and `&mut T` (write) in the same query is fine — `hecs` (the ECS crate Pebble builds on) borrow-checks it per-component at runtime, so this only panics if some *other* system concurrently holds a conflicting borrow, not merely because the query itself asks for both.

Include `Entity` directly in `Q` when you need the id back alongside the components — it's a query term like any other, not a separate mechanism:

```rust
fn despawn_far_away(mut commands: Commands, mut query: Query<(Entity, &Name, &Position)>) {
    for (entity, name, pos) in query.iter() {
        if pos.x.hypot(pos.y) > 10.0 {
            commands.despawn(entity);
        }
    }
}
```

Two more helpers for the cases where you don't want the whole result set:

- **`query.get(entity)`** — look up one known entity directly, without scanning the rest of the query. Returns `None` if the entity doesn't exist or doesn't match `Q`.
- **`query.single()`** / **`query.get_single()`** — expect exactly one match (the player, the active camera). `single` panics if that's not true; `get_single` returns `None` instead.

## Narrowing by component, and filtering by value

`query.with::<R>()`/`query.without::<R>()` narrow a query to entities that also/don't have component(s) `R`, without `R` itself joining the yielded items. Each returns another `Query`, so they chain — and every method above (`.iter()`, `.get()`, `.single()`, further `.with()`/`.without()`) still works on the result, no `hecs` types involved at any point:

```rust
fn low_health_enemies(mut query: Query<(&Name, &Health)>) {
    let mut enemies = query.with::<&Enemy>().without::<&Dead>();
    for (name, health) in enemies.iter() {
        if health.0 < 10 {
            println!("{} is dying", name.0);
        }
    }
}
```

`with`/`without` only filter by which components an entity *has* — for a predicate over their *values*, filter the iterator instead, since `.iter()` returns a plain `Iterator` that every standard adapter already works on:

```rust
let dying: Vec<_> = query.iter().filter(|(_, health)| health.0 < 10).collect();
```

## Commands: deferred mutation

`despawn_far_away` above calls `commands.despawn(entity)` *while a query over the same world is still borrowed* — safe only because `Commands` doesn't touch the world immediately. Every `Commands` call (`spawn`, `despawn`, `insert_resource`, ...) queues an operation into a command buffer, which is flushed once the current stage finishes running every system in it. This is also why a resource inserted via `commands.insert_resource(...)` isn't visible to a system *later in the same stage* — only from the next stage (or the next tick) onward; use `App::add_resource` directly (outside of a system) or `ResMut`/direct mutation inside a system when you need it visible immediately.

## Why the split?

Querying and mutating the same world simultaneously is exactly the kind of aliasing Rust's borrow checker exists to prevent — `Commands` sidesteps it by not aliasing at all: it just appends "do this later" to a buffer, and the actual mutation happens at a single well-defined point (the end of the stage) where nothing else is borrowed. It's the same trick most ECS frameworks use, under whatever name they give it (Bevy calls its version `Commands` too).
