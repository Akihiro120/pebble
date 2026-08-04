# Core API Reference

A task-indexed lookup for the ECS core — "how do I add a system," "how do I read a resource." For the concepts behind any of this, see [Part I](./ch01-app-and-plugins.md); this page just gets you to working code fast. Everything below is reachable via `use pebble::prelude::*;`.

## The App and Plugins

### Building an app

```rust
App::new()
    .add_plugin(MyWindowPlugin)
    .add_plugin(MyBackendPlugin)
    .add_plugin(MyGamePlugin)
    .build()
    .run();
```

### Writing a plugin

A `Plugin` is one method — add resources, register systems, or queue other plugins:

```rust
struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Time { delta: 0.0 })
            .add_system(SystemStage::PreUpdate, update_delta_time);
    }
}
```

`App::build()` drains queued plugins to convergence, then panics if any system's hard-requirement resources (see below) are never declared by anything — see [Chapter 1](./ch01-app-and-plugins.md).

## Systems

### Adding a system

Any function whose parameters are all `SystemParam`s — no registration macro:

```rust
fn move_system(time: Res<Time>, mut q: Query<&mut Position>) { /* ... */ }

app.add_system(SystemStage::Update, move_system);
```

Stages run in a fixed order each tick: `PreUpdate` → `Update` → `PostUpdate` → `PreRender` → `AssetSync`/`AssetSyncDeps` → `Render` → `PostRender`. Full table: [Chapter 2](./ch02-systems-and-resources.md#stages-when-a-system-runs).

### Registering several at once

```rust
app.add_systems(SystemStage::Update, (system_a, system_b, system_c));
```

### Ordering within a stage

Systems in the same stage otherwise run in registration order — `.before()`/`.after()` impose an explicit constraint instead:

```rust
app.add_systems(SystemStage::Update, (
    apply_input,
    move_player.after(apply_input),
));
```

### Run once

No dedicated "Startup" stage — `.once()` turns "have I already done this" into the return value: `None` means "call me again next tick," `Some(())` retires the system permanently.

```rust
fn spawn_scene(mut commands: Commands, config: Option<Res<MyConfig>>) -> Option<()> {
    let config = config?; // not ready yet
    commands.spawn(/* ... */);
    Some(())
}

app.add_system(SystemStage::PreUpdate, spawn_scene.once());
```

### Run conditions

`.run_if::<C>()` gates a system behind a `RunCondition`, re-checked every tick — parameters aren't even fetched unless it holds:

```rust
app.add_system(SystemStage::Update, expensive_diagnostic.run_if::<ResourceExists<DebugOverlay>>());
```

Built in: `ResourceExists<T>`, `And<A, B>`/`Or<A, B>`; implement `RunCondition` for anything else. Full discussion: [Chapter 2](./ch02-systems-and-resources.md#run-conditions).

## Resources

### Reading a resource — `Res<T>`

```rust
app.add_resource(MyConfig { volume: 0.8 });

fn my_system(config: Res<MyConfig>) {
    println!("{}", config.volume);
}
```

A bare `Res<T>`/`ResMut<T>` is a **hard requirement**: if nothing has declared it'll eventually provide `T`, `App::build()` panics at startup naming the system and the missing resource. If something has declared it (a `LazyResource`, an async backend) but hasn't produced it *yet*, the system is silently skipped and retried next tick — not an error. See [Chapter 2](./ch02-systems-and-resources.md#what-happens-when-a-resource-isnt-there-yet).

### Mutating a resource — `ResMut<T>`

```rust
fn my_system(mut config: ResMut<MyConfig>) {
    config.volume = 1.0;
}
```

Same `RefCell`-style borrow rules across a tick as `Res`/`ResMut` on any other type — two systems in the same stage both wanting `ResMut<T>` is fine (they run sequentially), but you can't hold a guard past the system call.

### A genuinely optional resource — `Option<Res<T>>`/`Option<ResMut<T>>`

Use when a resource may legitimately never exist (not "not ready yet") — never panics, never waits, just yields `None`:

```rust
fn maybe_render(backend: Option<Res<WGPUBackend>>) {
    let Some(backend) = backend else { return };
    // ...
}
```

### Per-system local state — `Local<T>`

Not shared through `Resources` — each system gets its own private `T: Default`, persisted across every subsequent run of that specific system:

```rust
fn count_ticks(mut count: Local<u32>) {
    *count += 1;
}
```

## Queries, Commands, Entities

### Spawning an entity

```rust
fn spawn_entities(mut commands: Commands) -> Option<()> {
    commands.spawn((Name("wanderer"), Position { x: 0.0, y: 0.0 }));
    Some(())
}
```

Any tuple of component types works — no registration step. `Commands` defers every mutation (`spawn`, `despawn`, `insert_resource`, ...) to a buffer flushed at the end of the current stage — see [Chapter 3](./ch03-queries-and-commands.md#why-the-split) for why.

### Querying components

```rust
fn move_entities(mut query: Query<(&Name, &mut Position, &Velocity)>) {
    for (name, pos, vel) in query.iter() {
        pos.x += vel.dx;
        pos.y += vel.dy;
    }
}
```

Mixing `&T`/`&mut T` in the same query is fine — borrow-checked per-component at runtime by `hecs`. Include `Entity` in `Q` to get the id back alongside components.

### Looking up specific entities

```rust
query.get(entity)        // -> Option<Q>, one known entity
query.single()           // panics unless exactly one match
query.get_single()       // -> Option<Q>, None instead of panicking
```

### Despawning

```rust
fn despawn_far_away(mut commands: Commands, query: Query<(Entity, &Position)>) {
    for (entity, pos) in query.iter() {
        if pos.x.hypot(pos.y) > 10.0 {
            commands.despawn(entity);
        }
    }
}
```

Safe to call while a query over the same world is still borrowed — `Commands` doesn't touch the world until the deferred flush.

## Events

### Registering, sending, reading

```rust
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

An event sent during tick `N` stays visible through all of `N + 1`, then is dropped — long enough that reader/writer registration order within a stage never causes a missed same-tick event. See [Chapter 4](./ch04-events.md#why-two-ticks).

### Optional reader/writer

Same shape as `Option<Res<T>>` — for library code that reacts to an event type the host app may not have registered:

```rust
fn maybe_log_damage(reader: Option<EventReader<Damage>>) {
    let Some(mut reader) = reader else { return };
    for event in reader.iter() { /* ... */ }
}
```

## Async and Background Tasks

Four ways to use `Res<BackgroundTasks>` (registered by `BackgroundTasksPlugin::new(worker_count)`), by what you need back:

| I want... | Use | Result delivery |
|---|---|---|
| A blocking closure off-thread, native only | `BackgroundTasks::spawn_blocking` | poll the `TaskHandle<T>` yourself |
| A future off-thread, web-compatible | `BackgroundTasks::spawn_async` | poll the `TaskHandle<T>` yourself |
| A whole fire-and-forget async system | `.detach()` | nothing |
| A future whose result becomes an event | `AsyncEventWriter<T>` | automatic, via `EventReader<T>` |

Full decision table with rationale: [Chapter 5](./ch05-async.md).

### The common case: deliver a result as an event

```rust
app.add_async_event::<ReadbackDone>(); // not add_event — a hint fires if you use the wrong one

fn start_readback(events: AsyncEventWriter<ReadbackDone>, buffer: Res<SomeGpuBuffer>) {
    let future = buffer.0.read(); // buffer.0: pebble::wgpu::buffer::Buffer
    events.spawn(async move { ReadbackDone(future.await) });
}

fn on_readback(mut reader: EventReader<ReadbackDone>) {
    for event in reader.iter() { /* event.0 */ }
}
```

### Fire-and-forget: `.detach()`

```rust
fn save_screenshot(tasks: Res<BackgroundTasks>) -> impl Future<Output = ()> + Send + 'static {
    let tasks = tasks.clone();
    async move { /* ... write to disk ... */ }
}

app.add_system(SystemStage::Update, save_screenshot.detach());
```

The system body runs synchronously (parameters fetched normally) and returns a future instead of doing the work directly — a real `async fn` can't be used here since its future borrows the parameters and is never `'static` on its own.

### Polling a task yourself

```rust
let handle: TaskHandle<Vec<u8>> = tasks.spawn_blocking(|| expensive_computation());

// later, once per tick:
match handle.poll() {
    TaskStatus::Pending => {}                          // not done yet, check again next tick
    TaskStatus::Ready(value) => { /* use value */ }
    TaskStatus::Panicked(message) => tracing::error!("task failed: {message}"),
}
```

`spawn_blocking` is native-only (no OS thread in a browser tab); `spawn_async` takes a future and works on both. `TaskStatus::Panicked` — not just a `None`/silent hang — is why `poll()` is preferred over the older `try_recv()`.

### A browser event reaching your systems

No future to await for a DOM callback — a plain channel filled by a `wasm_bindgen` closure, drained into an `EventWriter` by an ordinary system. Full pattern: [Chapter 5](./ch05-async.md#getting-a-js-event-into-the-scheduler).
