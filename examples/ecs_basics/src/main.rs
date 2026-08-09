//! `ecs_playground` — a tour of Pebble's ECS: components, resources,
//! queries, commands, `Local<T>`, startup systems, `run_if`, events, and
//! async systems — all in one self-contained program, no window, no
//! graphics backend.
//!
//! Run with: `cargo run`
//!
//! Read the numbered sections in order. Each demonstrates exactly one
//! concept, building on the ones before it.

use pebble::prelude::*;

// =======================================================================
// COMPONENTS — per-entity data. Each entity has its own copy of whatever
// components it was spawned with.
// =======================================================================

struct Position {
    x: f32,
    y: f32,
}

struct Velocity {
    dx: f32,
    dy: f32,
}

struct Name(&'static str);

/// A zero-data marker component — used purely to tag entities so a query
/// can filter by "has this tag", regardless of what else it carries.
struct Asleep;

// =======================================================================
// RESOURCES — global, shared state. Exactly one instance of each type,
// not tied to any entity. Accessed via `Res<T>` (read) / `ResMut<T>`
// (read + write).
// =======================================================================

struct TickCount(u32);

// =======================================================================
// DEFERRED RESOURCE INIT — a resource that can only be built once another
// resource (a "backend") exists. The pattern: a Startup system that returns
// Option<()> — it retries every tick until its dependencies are present, then
// inserts the resource and returns Some(()), retiring itself. Here we simulate
// a backend that only becomes available after a few ticks, so you can see the
// waiting behavior for yourself.

/// Stands in for something like a graphics backend: not available
/// immediately, appears after a few ticks.
struct FakeBackend;

/// Something that can only be built once `FakeBackend` exists.
struct ExpensiveSetup {
    built_on_tick: u32,
}

fn init_expensive_setup(backend: Option<Res<FakeBackend>>, mut commands: Commands) -> Option<()> {
    let _backend = backend?;
    commands.insert_resource(ExpensiveSetup { built_on_tick: 0 });
    Some(())
}

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_resource(TickCount(0))
        .add_plugin(BackgroundTasksPlugin::new(2))
        .add_event::<Milestone>()
        .add_async_event::<ComputeResult>()
        .add_system(SystemStage::Startup, spawn_entities)
        .add_system(SystemStage::Startup, init_expensive_setup)
        .add_systems(
            SystemStage::Update,
            (
                print_tick_header,
                move_entities,
                count_asleep,
                despawn_far_away,
                reveal_fake_backend,
                // `run_if` — this system's body only runs (and its
                // SystemParams are only fetched) once ExpensiveSetup
                // actually exists. Before that, it's skipped entirely,
                // no Option<Res<...>> needed inside it at all.
                report_expensive_setup.run_if::<ResourceExists<ExpensiveSetup>>(),
            ),
        )
        .add_system(SystemStage::Update, announce_halfway)
        .add_system(SystemStage::Update, print_milestones)
        .add_system(SystemStage::Update, kick_off_background_computation)
        .add_system(SystemStage::Update, print_computation_result)
        .set_runner(|mut app| {
            const TOTAL_TICKS: u32 = 30;
            for _ in 0..TOTAL_TICKS {
                app.update();
                // The background-computation demo (section 8) runs on a
                // real worker thread, off this loop — a short real-time
                // sleep between ticks gives it a chance to actually finish
                // before the program exits. Not needed for anything else
                // here; every other system in this file only cares about
                // tick count, not wall-clock time.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        })
        .build()
        .run();
}

// =======================================================================
// 1. SPAWNING — functions returning Option<()> automatically run-once: they
//    retry each tick until they return Some(()), then retire. Here this
//    always succeeds on the very first Startup tick.
// =======================================================================

fn spawn_entities(mut commands: Commands) -> Option<()> {
    commands.spawn((
        Name("wanderer"),
        Position { x: 0.0, y: 0.0 },
        Velocity { dx: 1.0, dy: 2.0 },
    ));

    commands.spawn((
        Name("sleeper"),
        Position { x: 3.0, y: 3.0 },
        Velocity { dx: 0.0, dy: 0.0 },
        Asleep,
    ));

    commands.spawn((
        Name("racer"),
        Position { x: 0.0, y: 0.0 },
        Velocity { dx: 4.0, dy: 4.0 }, // fast enough to trigger despawn later
    ));

    println!("--- spawned 3 entities: wanderer, sleeper, racer ---\n");
    Some(())
}

// =======================================================================
// 2. RESOURCES — shared state every system can reach, independent of
//    any one entity.
// =======================================================================

fn print_tick_header(mut tick: ResMut<TickCount>) {
    tick.0 += 1;
    println!("== tick {} ==", tick.0);
}

// =======================================================================
// 3. QUERIES — iterate every entity matching a set of components,
//    mutating some, reading others, in the same pass.
// =======================================================================

fn move_entities(mut query: Query<(&Name, &mut Position, &Velocity)>) {
    for (name, pos, vel) in query.iter() {
        pos.x += vel.dx;
        pos.y += vel.dy;
        println!("  {} moved to ({:.1}, {:.1})", name.0, pos.x, pos.y);
    }
}

// =======================================================================
// 4. MARKER COMPONENTS — filtering a query by "has this tag" alone,
//    since `Asleep` carries no data of its own.
// =======================================================================

fn count_asleep(mut query: Query<(&Name, &Asleep)>) {
    for (name, _) in query.iter() {
        println!("  {} is asleep, skipping", name.0);
    }
}

// =======================================================================
// 5. COMMANDS FROM INSIDE A SYSTEM — despawning while a query is still
//    borrowed. Safe because Commands defers the actual despawn until
//    after this stage finishes.
// =======================================================================

fn despawn_far_away(mut commands: Commands, mut query: Query<(Entity, &Name, &Position)>) {
    const RANGE: f32 = 10.0;

    for (entity, name, pos) in query.iter() {
        let distance = (pos.x * pos.x + pos.y * pos.y).sqrt();
        if distance > RANGE {
            println!(
                "  {} went past {RANGE:.0} units (at {distance:.1}) -- despawning",
                name.0
            );
            commands.despawn(entity);
        }
    }
}

// =======================================================================
// 6. DEFERRED RESOURCE INIT IN ACTION — `FakeBackend` doesn't exist until
//    tick 3. Until then, `init_expensive_setup` (a Startup system) returns
//    None each tick — it retries automatically. Once FakeBackend appears,
//    it succeeds and inserts ExpensiveSetup, then retires. Nothing that
//    depends on ExpensiveSetup needs to poll for it manually.
// =======================================================================

fn reveal_fake_backend(
    mut commands: Commands,
    tick: Res<TickCount>,
    backend: Option<Res<FakeBackend>>,
) {
    if backend.is_none() && tick.0 == 3 {
        println!("  (FakeBackend just became available)");
        commands.insert_resource(FakeBackend);
    }
}

/// Only ever called once `ExpensiveSetup` exists, thanks to `run_if`
/// above — this system's own body never has to check for that itself.
fn report_expensive_setup(
    setup: Res<ExpensiveSetup>,
    tick: Res<TickCount>,
    mut announced: Local<bool>,
) {
    if !*announced {
        println!(
            "  ExpensiveSetup is ready! (LazyResource waited for FakeBackend, then built itself, tick {})",
            tick.0
        );
        *announced = true;
    }
    let _ = setup.built_on_tick; // field exists to show LazyResource output carries real data
}

// =======================================================================
// 7. EVENTS — a double-buffered queue, distinct from a resource: an event
//    sent during tick N stays visible to every reader for the rest of N
//    and all of N + 1, then is dropped, so a reader sees it exactly once
//    regardless of whether it runs before or after the writer. Each
//    `EventReader<T>` keeps its own private read cursor (like `Local<T>`),
//    so independent readers of the same event type don't interfere.
// =======================================================================

struct Milestone(&'static str);

fn announce_halfway(tick: Res<TickCount>, mut writer: EventWriter<Milestone>, mut sent: Local<bool>) {
    if tick.0 == 4 && !*sent {
        writer.send(Milestone("halfway through the run"));
        *sent = true;
    }
}

fn print_milestones(mut reader: EventReader<Milestone>) {
    for milestone in reader.iter() {
        println!("  [event] {}", milestone.0);
    }
}

// =======================================================================
// 8. ASYNC SYSTEMS & BACKGROUND TASKS — `BackgroundTasksPlugin` runs a
//    small worker-thread pool. `AsyncEventWriter<T>` spawns a future onto it
//    and delivers the result as a `T` event once it resolves — no manual
//    polling, no `TaskHandle` to hold onto, just an ordinary `EventReader`
//    on the other end. (The lower-level pieces this is built from —
//    `BackgroundTasks::spawn_blocking`/`spawn_async`, and fire-and-forget async
//    systems via `.detach()` — are documented on those items directly and
//    in the Readme's "Async systems & background tasks" section.)
// =======================================================================

struct ComputeResult(u32);

fn kick_off_background_computation(
    events: AsyncEventWriter<ComputeResult>,
    tick: Res<TickCount>,
    mut sent: Local<bool>,
) {
    if tick.0 == 2 && !*sent {
        println!("  [async] kicking off a background computation...");
        events.spawn(async {
            // Stands in for real work (a file load, a GPU readback, ...) —
            // just enough of a delay to make it obvious this isn't
            // computed inline on the main tick loop.
            std::thread::sleep(std::time::Duration::from_millis(30));
            ComputeResult(6 * 7)
        });
        *sent = true;
    }
}

fn print_computation_result(mut reader: EventReader<ComputeResult>) {
    for result in reader.iter() {
        println!(
            "  [async] background computation finished: the answer is {}",
            result.0
        );
    }
}
