use std::cell::Cell;
use std::rc::Rc;

use pebble::prelude::*;

struct ResA;
struct ResB;
struct ResC;

/// A two-hop dependency chain (C depends on B depends on A) resolves within
/// a single `App::build()` call, proving the `AssetSyncDeps` convergence
/// loop keeps re-running the stage until no new resources appear.
#[test]
fn convergent_stage_resolves_multi_step_chain_in_one_tick() {
    let mut app = App::new();
    let saw_c = Rc::new(Cell::new(false));

    app.add_system(SystemStage::Startup, |mut cmds: Commands| {
        cmds.insert_resource(ResA);
    });

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
        "ResC should exist after one build()+update() — convergence loop \
         should have resolved the A -> B -> C chain within a single tick"
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
        "Update ran during build() (via startup/asset convergence) or looped \
         within a single update() call, but it should run exactly once per update()"
    );
}

/// A `Startup` system with a hard `Res<T>` requirement on a resource
/// produced by an earlier `Startup` system (via deferred `Commands`, only
/// applied after that system's own pass) must wait for a later pass rather
/// than panicking, then fire exactly once — never again on subsequent passes
/// or ticks.
#[test]
fn startup_waits_for_hard_requirement_then_fires_exactly_once() {
    let mut app = App::new();
    let saw_b = Rc::new(Cell::new(false));
    let run_count = Rc::new(Cell::new(0u32));

    app.add_system(SystemStage::Startup, |mut cmds: Commands| {
        cmds.insert_resource(ResA);
    });

    {
        let saw_b = saw_b.clone();
        let run_count = run_count.clone();
        app.add_system(
            SystemStage::Startup,
            move |_a: Res<ResA>, mut cmds: Commands| {
                run_count.set(run_count.get() + 1);
                cmds.insert_resource(ResB);
                saw_b.set(true);
            },
        );
    }

    app.build();
    app.update();
    app.update();

    assert!(
        saw_b.get(),
        "ResB-producing system should see ResA within build() — it should be \
         skipped (not panicked) until ResA is flushed, then run on a later pass"
    );
    assert_eq!(
        run_count.get(),
        1,
        "a Startup system must fire exactly once, ever, even across later update() ticks"
    );
}

/// A `Startup` system whose hard requirement is never satisfied must not
/// panic — it should simply stay pending indefinitely, retried every tick,
/// while everything else keeps running normally.
#[test]
fn startup_system_with_unmet_requirement_is_skipped_not_panicked() {
    let mut app = App::new();
    let update_ran = Rc::new(Cell::new(false));

    app.add_system(SystemStage::Startup, |_a: Res<ResA>| {
        panic!("should never run — ResA is never inserted");
    });

    {
        let update_ran = update_ran.clone();
        app.add_system(SystemStage::Update, move || {
            update_ran.set(true);
        });
    }

    app.build();
    app.update();

    assert!(
        update_ran.get(),
        "Update should run normally even though a Startup system is permanently pending"
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

/// `.run_if::<ResourceExists<T>>()` must fully exempt a system from the
/// pre-flight check, even when the underlying resource is never registered
/// via `App::provides` — the condition itself is trusted to gate correctly.
#[test]
fn run_if_gated_system_never_triggers_missing_resource_panic() {
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
        "gated system should never have run — ResA is never inserted, and that's fine"
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
