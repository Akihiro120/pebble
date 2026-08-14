use std::any::TypeId;

use crate::ecs::{
    commands::{ResourceCommandQueue, TriggerQueue},
    resources::Resources,
    system_param::{IntoSystem, System, SystemConfig},
};

/// An ordered list of systems, run together — one `Schedule` backs each
/// [`SystemStage`](crate::ecs::system::SystemStage). Running it also
/// flushes any deferred `Commands` from the systems that just ran.
///
/// Systems normally run in the order they were added. Call `.after(...)`/
/// `.before(...)` directly on a system (see
/// [`IntoSystemConfig`](crate::ecs::system_param::IntoSystemConfig)) to
/// constrain it relative to another system known to the schedule — added
/// earlier or later, registration order doesn't matter, only the
/// constraint does:
///
/// ```ignore
/// schedule
///     .add_system(spawn_enemies)
///     .add_system(move_enemies.after(spawn_enemies))
///     .add_system(render.after(move_enemies));
/// ```
#[derive(Default)]
pub struct Schedule {
    systems: Vec<(TypeId, Box<dyn System>)>,
    /// `(dependent, dependency)` — `dependent` must run after `dependency`.
    constraints: Vec<(TypeId, TypeId)>,
    order: Vec<usize>,
    order_dirty: bool,
}

impl Schedule {
    /// Appends `system` to the end of this schedule. `system` may be a bare
    /// system, or one wrapped with `.after(...)`/`.before(...)` to
    /// constrain it relative to another system in this schedule.
    pub fn add_system<S, Params>(&mut self, system: impl Into<SystemConfig<S, Params>>) -> &mut Self
    where
        Params: 'static,
        S: IntoSystem<Params> + 'static,
    {
        let (id, system, constraints) = system.into().into_parts();
        self.systems.push((id, system));
        self.constraints.extend(constraints);
        self.order_dirty = true;
        self
    }

    /// Topologically sorts systems to satisfy every `after`/`before`
    /// constraint, breaking ties by original `add_system` order. Panics if
    /// the constraints form a cycle; silently ignores a constraint that
    /// names a system never added to this schedule.
    fn compute_order(&self) -> Vec<usize> {
        let n = self.systems.len();
        let index_of = |id: TypeId| self.systems.iter().position(|(sid, _)| *sid == id);

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
            let ready = remaining
                .iter()
                .position(|&i| in_degree[i] == 0)
                .expect("system ordering constraints form a cycle");
            let picked = remaining.remove(ready);
            order.push(picked);
            for &dependent in &dependents[picked] {
                in_degree[dependent] -= 1;
            }
        }

        order
    }

    /// Runs every system in order, then flushes deferred entity spawns,
    /// resource commands, and triggered observers from this run.
    pub fn run(&mut self, world: &mut hecs::World, resources: &mut Resources) {
        if self.order_dirty {
            self.order = self.compute_order();
            self.order_dirty = false;
        }

        for &index in &self.order {
            self.systems[index].1.run(world, &*resources);
        }

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
