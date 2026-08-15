use std::any::TypeId;

use crate::ecs::{
    commands::{ResourceCommandQueue, TriggerQueue},
    resources::Resources,
    system_param::{IntoSystem, System, SystemChain, SystemConfig},
};

/// An ordered list of systems, run together — one `Schedule` backs each
/// [`SystemStage`](crate::ecs::system::SystemStage). Deferred `Commands`
/// (entity spawns, resource inserts/removes, triggers) are flushed after
/// *each* system, not just once at the end — so a system ordered
/// `.after(...)` another can rely on seeing its `Commands` effects, even
/// within the same stage.
///
/// Systems normally run in the order they were added. Call `.after(...)`/
/// `.before(...)` directly on a system (see
/// [`IntoSystemConfig`](crate::ecs::system_param::IntoSystemConfig)) to
/// constrain it relative to another system known to the schedule — added
/// earlier or later, registration order doesn't matter, only the
/// constraint does. `.priority(...)` breaks ties between systems with no
/// `after`/`before` relationship to each other — higher runs first — but
/// never overrides an explicit constraint. `.chain()` on a tuple of systems
/// (see [`Chain`](crate::ecs::system_param::Chain), registered with
/// [`add_systems`](Schedule::add_systems)) forces them to run in that exact
/// relative order, and can itself be given `.after(...)`/`.before(...)`/
/// `.priority(...)`, applied to the whole chain:
///
/// ```ignore
/// schedule
///     .add_system(spawn_enemies)
///     .add_system(move_enemies.after(spawn_enemies))
///     .add_system(render.after(move_enemies))
///     .add_system(hud.priority(10))
///     .add_systems((physics_step, resolve_collisions).chain().before(render));
/// ```
#[derive(Default)]
pub struct Schedule {
    systems: Vec<(TypeId, Box<dyn System>, i32)>,
    /// `(dependent, dependency)` — `dependent` must run after `dependency`.
    constraints: Vec<(TypeId, TypeId)>,
    order: Vec<usize>,
    order_dirty: bool,
}

impl Schedule {
    /// Appends `system` to the end of this schedule. `system` may be a bare
    /// system, or one wrapped with `.after(...)`/`.before(...)`/`.priority(...)`
    /// — see [`IntoSystemConfig`](crate::ecs::system_param::IntoSystemConfig).
    pub fn add_system<S, Params>(&mut self, system: impl Into<SystemConfig<S, Params>>) -> &mut Self
    where
        Params: 'static,
        S: IntoSystem<Params> + 'static,
    {
        let (id, system, priority, constraints) = system.into().into_parts();
        self.systems.push((id, system, priority));
        self.constraints.extend(constraints);
        self.order_dirty = true;
        self
    }

    /// Appends every system in `chain` (built with `.chain()` on a tuple of
    /// systems — see [`Chain`](crate::ecs::system_param::Chain)) to the end
    /// of this schedule.
    pub fn add_systems(&mut self, chain: SystemChain) -> &mut Self {
        let (systems, constraints) = chain.into_parts();
        self.systems.extend(systems);
        self.constraints.extend(constraints);
        self.order_dirty = true;
        self
    }

    /// Topologically sorts systems to satisfy every `after`/`before`
    /// constraint. Among systems with no constraint relative to each other,
    /// higher `priority` runs first; ties within the same priority break by
    /// original `add_system` order. Panics if the constraints form a cycle;
    /// silently ignores a constraint that names a system never added to
    /// this schedule.
    fn compute_order(&self) -> Vec<usize> {
        let n = self.systems.len();
        let index_of = |id: TypeId| self.systems.iter().position(|(sid, _, _)| *sid == id);

        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for &(dependent_id, dependency_id) in &self.constraints {
            if let (Some(dependent), Some(dependency)) =
                (index_of(dependent_id), index_of(dependency_id))
                && dependent != dependency
            {
                dependents[dependency].push(dependent);
                in_degree[dependent] += 1;
            }
        }

        let mut remaining: Vec<usize> = (0..n).collect();
        let mut order = Vec::with_capacity(n);

        while !remaining.is_empty() {
            // among ready systems (in_degree 0), pick the highest priority;
            // ties keep the first one found, preserving `add_system` order
            // since `remaining` is only ever shrunk, never reordered.
            let mut best: Option<(usize, i32)> = None;
            for (pos, &i) in remaining.iter().enumerate() {
                if in_degree[i] != 0 {
                    continue;
                }
                let priority = self.systems[i].2;
                if best.is_none_or(|(_, best_priority)| priority > best_priority) {
                    best = Some((pos, priority));
                }
            }
            let ready = best.map(|(pos, _)| pos).expect("system ordering constraints form a cycle");
            let picked = remaining.remove(ready);
            order.push(picked);
            for &dependent in &dependents[picked] {
                in_degree[dependent] -= 1;
            }
        }

        order
    }

    /// Runs every system in order, flushing deferred entity spawns, resource
    /// commands, and triggered observers after *each* system — not once at
    /// the end of the whole schedule. This is what lets a system that reads
    /// a resource another system just inserted via `Commands::insert_resource`
    /// see it immediately, as long as it's ordered `.after(...)` the system
    /// that queued it — the two don't need to be in different stages.
    pub fn run(&mut self, world: &mut hecs::World, resources: &mut Resources) {
        if self.order_dirty {
            self.order = self.compute_order();
            self.order_dirty = false;
        }

        for &index in &self.order {
            self.systems[index].1.run(world, &*resources);
            Self::sync_commands(world, resources);
        }
    }

    /// Applies the entity/resource commands and triggered observers queued
    /// by the system that just ran, so the next system in this schedule
    /// observes them.
    fn sync_commands(world: &mut hecs::World, resources: &mut Resources) {
        // sync entity commands
        resources.get_mut::<hecs::CommandBuffer>().run_on(world);

        // sync resource commands
        if resources.contains::<ResourceCommandQueue>() {
            let commands = std::mem::take(&mut resources.get_mut::<ResourceCommandQueue>().0);
            for command in commands {
                command(resources);
            }
        }

        // sync triggered observers
        if resources.contains::<TriggerQueue>() {
            let triggers = std::mem::take(&mut resources.get_mut::<TriggerQueue>().0);
            for trigger in triggers {
                trigger(world, resources);
            }
        }
    }
}
