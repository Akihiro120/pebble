use std::cell::Cell;
use std::rc::Rc;

use pebble::prelude::*;

struct ResA;
struct ResB;
struct ResC;

/// A two-hop dependency chain (C depends on B depends on A) resolves within
/// a single `update()` tick, proving the `AssetSyncDeps` convergence loop
/// keeps re-running the stage until no new resources appear, and that it
/// picks up a resource produced earlier in the same tick by a regular
/// `PreUpdate` system.
#[test]
fn convergent_stage_resolves_multi_step_chain_in_one_tick() {
    let mut app = App::new();
    let saw_c = Rc::new(Cell::new(false));

    app.add_system(
        SystemStage::PreUpdate,
        (|mut cmds: Commands| {
            cmds.insert_resource(ResA);
            Some(())
        })
        .once(),
    );

    app.add_system(
        SystemStage::AssetSyncDeps,
        |a: Option<Res<ResA>>, b: Option<Res<ResB>>, mut cmds: Commands| {
            if a.is_some() && b.is_none() {
                cmds.insert_resource(ResB);
            }
        },
    );
    app.add_system(
        SystemStage::AssetSyncDeps,
        |b: Option<Res<ResB>>, c: Option<Res<ResC>>, mut cmds: Commands| {
            if b.is_some() && c.is_none() {
                cmds.insert_resource(ResC);
            }
        },
    );

    {
        let saw_c = saw_c.clone();
        app.add_system(SystemStage::Update, move |c: Option<Res<ResC>>| {
            if c.is_some() {
                saw_c.set(true);
            }
        });
    }

    app.build();
    app.update();

    assert!(
        saw_c.get(),
        "ResC should exist after one update() tick — PreUpdate produces ResA, the \
         reconverge() right after PreUpdate should resolve the A -> B -> C chain \
         through AssetSyncDeps before Update runs"
    );
}

/// Non-convergent stages must NOT loop: a system that keeps inserting a
/// resource every pass should still only run once per `update()` since
/// `Update` isn't in `SystemStage::is_convergent()`.
#[test]
fn non_convergent_stage_runs_exactly_once_per_tick() {
    let mut app = App::new();
    let run_count = Rc::new(Cell::new(0u32));

    {
        let run_count = run_count.clone();
        app.add_system(SystemStage::Update, move |mut cmds: Commands| {
            run_count.set(run_count.get() + 1);
            cmds.insert_resource(ResA);
        });
    }

    app.build();
    app.update();
    app.update();

    assert_eq!(
        run_count.get(),
        2,
        "Update ran during build() (via asset convergence) or looped within a \
         single update() call, but it should run exactly once per update()"
    );
}

/// `.once()` is the replacement for a dedicated "Startup" stage: the system
/// keeps being invoked every tick, on whichever stage it's registered to,
/// until it returns `Some(())` — at which point it retires permanently.
/// Returning `None` (here because `ResA`, produced by another `.once()`
/// system via deferred `Commands`, isn't visible until the next tick) means
/// "not ready, try again" rather than "done".
#[test]
fn once_waits_for_dependency_then_fires_exactly_once() {
    let mut app = App::new();
    let saw_b = Rc::new(Cell::new(false));
    let run_count = Rc::new(Cell::new(0u32));

    app.add_system(
        SystemStage::PreUpdate,
        (|mut cmds: Commands| {
            cmds.insert_resource(ResA);
            Some(())
        })
        .once(),
    );

    {
        let saw_b = saw_b.clone();
        let run_count = run_count.clone();
        app.add_system(
            SystemStage::PreUpdate,
            (move |a: Option<Res<ResA>>, mut cmds: Commands| {
                let _a = a?;
                run_count.set(run_count.get() + 1);
                cmds.insert_resource(ResB);
                saw_b.set(true);
                Some(())
            })
            .once(),
        );
    }

    app.build();
    app.update();
    app.update();
    app.update();

    assert!(
        saw_b.get(),
        "ResB-producing system should eventually see ResA — it should keep \
         returning None (try again) until ResA is visible, then run"
    );
    assert_eq!(
        run_count.get(),
        1,
        "a .once() system must fire exactly once, ever, even across later update() ticks"
    );
}

/// A `.once()` system that keeps returning `None` forever (its dependency
/// never appears) must not panic and must not block anything else — it just
/// stays pending, retried every tick, indefinitely.
#[test]
fn once_system_that_never_succeeds_is_retried_not_panicked() {
    let mut app = App::new();
    let update_ran = Rc::new(Cell::new(false));
    let attempts = Rc::new(Cell::new(0u32));

    {
        let attempts = attempts.clone();
        app.add_system(
            SystemStage::Update,
            (move |_a: Option<Res<ResA>>| {
                attempts.set(attempts.get() + 1);
                None // ResA is never inserted — never actually finishes
            })
            .once(),
        );
    }

    {
        let update_ran = update_ran.clone();
        app.add_system(SystemStage::Update, move || {
            update_ran.set(true);
        });
    }

    app.build();
    app.update();
    app.update();

    assert!(
        update_ran.get(),
        "the other Update system should run normally even though a .once() system \
         is permanently pending"
    );
    assert!(
        attempts.get() >= 2,
        "a pending .once() system should keep being retried every tick, not just once"
    );
}

/// A non-convergent stage (`Update`) must panic up front — before any system
/// in the stage runs — if a system declares a hard `Res<T>` requirement on a
/// resource that was never inserted.
#[test]
#[should_panic(expected = "ResA")]
fn missing_hard_requirement_panics_before_stage_runs() {
    let mut app = App::new();
    app.add_system(SystemStage::Update, |_a: Res<ResA>| {});
    app.build();
    app.update();
}

/// A non-`Startup` stage must NOT panic on a missing hard requirement if the
/// resource is registered via `App::provides` — it should wait quietly, the
/// same way a `LazyResource` or an async GPU backend does, and pick up the
/// system once the resource actually appears.
#[test]
fn provided_but_not_ready_resource_waits_instead_of_panicking() {
    let mut app = App::new();
    let run_count = Rc::new(Cell::new(0u32));

    app.provides::<ResA>();

    {
        let run_count = run_count.clone();
        app.add_system(SystemStage::Update, move |_a: Res<ResA>| {
            run_count.set(run_count.get() + 1);
        });
    }

    app.build();
    app.update();
    app.update();

    assert_eq!(
        run_count.get(),
        0,
        "system should never have run — ResA was declared provided but never actually inserted"
    );

    app.add_resource(ResA);
    app.update();

    assert_eq!(
        run_count.get(),
        1,
        "system should run once ResA is actually inserted, having waited quietly until then"
    );
}

/// `.run_if::<ResourceExists<T>>()` gates a system every tick — it must
/// fully exempt it from the pre-flight check, even when the underlying
/// resource is never registered via `App::provides` (the condition itself is
/// trusted to gate correctly), and it must keep re-checking the condition
/// every tick rather than retiring after the first check.
#[test]
fn run_if_gates_every_tick_without_retiring() {
    let mut app = App::new();
    let run_count = Rc::new(Cell::new(0u32));

    {
        let run_count = run_count.clone();
        app.add_system(
            SystemStage::Update,
            (move |_a: Res<ResA>| {
                run_count.set(run_count.get() + 1);
            })
            .run_if::<ResourceExists<ResA>>(),
        );
    }

    app.build();
    app.update();
    app.update();

    assert_eq!(
        run_count.get(),
        0,
        "gated system should never have run — ResA is never inserted, and that's fine, \
         not a panic"
    );

    app.add_resource(ResA);
    app.update();
    app.update();

    assert_eq!(
        run_count.get(),
        2,
        "once ResA exists, the gated system should run on every subsequent tick — \
         run_if does not retire the system after the condition first holds"
    );
}

/// The closure-based counterpart to `.run_if()`: `.run_if_fn(...)` needs no
/// struct/impl boilerplate for a one-off condition, but must gate identically
/// — every tick, without retiring.
#[test]
fn run_if_fn_gates_every_tick_without_retiring() {
    let mut app = App::new();
    let run_count = Rc::new(Cell::new(0u32));

    {
        let run_count = run_count.clone();
        app.add_system(
            SystemStage::Update,
            (move |_a: Res<ResA>| {
                run_count.set(run_count.get() + 1);
            })
            .run_if_fn(|world, resources| resources.has_resource::<ResA>(world)),
        );
    }

    app.build();
    app.update();

    assert_eq!(run_count.get(), 0, "ResA is missing — condition false, no run");

    app.add_resource(ResA);
    app.update();
    app.update();

    assert_eq!(
        run_count.get(),
        2,
        "once the closure's condition holds, the system should run on every tick"
    );
}

/// A dependency that can never resolve must not hang App::build() — the
/// convergence loop should give up after its pass cap instead of looping
/// forever.
#[test]
fn permanently_unsatisfiable_dependency_does_not_hang_build() {
    let mut app = App::new();
    let pass_count = Rc::new(Cell::new(0u32));

    {
        let pass_count = pass_count.clone();
        app.add_system(SystemStage::AssetSyncDeps, move |mut cmds: Commands| {
            pass_count.set(pass_count.get() + 1);
            // Always inserts a fresh resource type carrying the current pass
            // count, so the generation counter bumps every pass forever.
            cmds.insert_resource(pass_count.get());
        });
    }

    app.build();

    assert_eq!(
        pass_count.get(),
        64,
        "convergence loop should stop at max_passes (64) instead of looping forever"
    );
}
