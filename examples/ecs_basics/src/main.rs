//! `ecs_playground` — a tour of Pebble's ECS: components, resources,
//! queries, commands, `Local<T>`, `LazyResource`, and `run_if` — all in
//! one self-contained program, no window, no graphics backend.
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
// LAZY RESOURCES — like a resource, but constructed on demand once its
// dependencies are actually ready, instead of being inserted up front.
// Real uses (elsewhere in a real app) are things like a GPU buffer that
// needs a graphics backend to exist first. Here we simulate that with a
// fake "backend" that only becomes available after a short delay, so you
// can see the waiting behavior for yourself.

/// Stands in for something like a graphics backend: not available
/// immediately, appears after a few ticks.
struct FakeBackend;

/// Something that can only be built once `FakeBackend` exists — this is
/// the `LazyResource`. It waits automatically; nothing needs to poll it
/// manually.
struct ExpensiveSetup {
    built_on_tick: u32,
}

impl LazyResource<FakeBackend> for ExpensiveSetup {
    type Deps<'a> = (); // doesn't need anything besides FakeBackend itself

    fn construct<'a>(_backend: &FakeBackend, _deps: &()) -> Option<Self> {
        Some(ExpensiveSetup { built_on_tick: 0 }) // tick filled in by whoever reads it
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    App::new()
        .add_resource(TickCount(0))
        .add_plugin(LazyResourcePlugin::<FakeBackend, ExpensiveSetup>::new())
        .add_system(SystemStage::PreUpdate, spawn_entities.once())
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
        .set_runner(|mut app| {
            const TOTAL_TICKS: u32 = 8;
            for _ in 0..TOTAL_TICKS {
                app.update();
            }
        })
        .build()
        .run();
}

// =======================================================================
// 1. SPAWNING — `.once()` runs a system until it returns Some(()), then
//    never again. Here it always succeeds on the very first tick.
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
// 6. LAZY RESOURCES IN ACTION — `FakeBackend` doesn't exist until tick 3.
//    Until then, `ExpensiveSetup` (which needs it) simply isn't built
//    yet, and nothing that depends on it runs. No manual polling, no
//    panics, no Option juggling needed by the systems that use it.
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
