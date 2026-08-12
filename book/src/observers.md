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

## Why multiple observers on one trigger is the actual point

The useful case isn't one observer — it's several unrelated ones reacting to the same moment without knowing about each other. Say an enemy dies: something should add score, something should roll loot, something should kick off a screen shake. Those three concerns have no reason to live in the same system, or even know the others exist:

```rust,ignore
struct EnemyDied { position: glam::Vec3, value: u32 }

fn award_score(trigger: Trigger<EnemyDied>, mut score: Write<Score>) {
    score.0 += trigger.value;
}

fn spawn_loot(trigger: Trigger<EnemyDied>, mut commands: Commands) {
    commands.spawn((Position(trigger.position), Loot));
}

fn trigger_screen_shake(trigger: Trigger<EnemyDied>, mut shake: Write<ScreenShake>) {
    shake.0 = 0.3;
}

app.add_observer(award_score)
    .add_observer(spawn_loot)
    .add_observer(trigger_screen_shake)
```

The system that actually detects the death doesn't call any of these directly — it just triggers the fact that happened:

```rust,ignore
use pebble::ecs::Entity;

fn check_deaths(mut q: Query<(Entity, &Health, &Value, &Position)>, mut commands: Commands) {
    for (entity, health, value, pos) in q.iter() {
        if health.0 <= 0 {
            commands.trigger(EnemyDied { position: pos.0, value: value.0 });
            commands.despawn(entity);
        }
    }
}
```

(See [Queries, Commands, and Entities](./queries-commands-entities.md#getting-entity-ids-out-of-a-query) for why `Entity` has to be named explicitly in the query to get it back out.)

Add a fourth reaction later (an achievement check, a kill-streak counter) by registering one more observer — `check_deaths` never changes. This is the same shape [Events](./events.md) gives you for many-to-many communication, but with a guarantee events don't make: every one of these three observers has already run, and their effects (like `award_score`'s mutation) are visible, before the stage that triggered the death finishes — so a UI system later in that same stage already sees the updated score, not one tick stale.

## Events vs. Observers

Use **Events** when several systems might poll for something whenever convenient, and it's fine if a couple ticks pass before someone reads it. Use **Observers** when you need a guaranteed, same-tick reaction — the trigger and its handling should feel like one atomic step, as in the death example above.
