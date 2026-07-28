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
