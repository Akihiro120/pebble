/// A handle to a specific entity in the ECS world. Pebble's own re-export
/// of `hecs::Entity` — since `hecs` is the ECS world itself, not a
/// swappable backend, referencing it doesn't require adding `hecs` as your
/// own dependency. Include it in a query tuple (`Query<(Entity, &Health)>`)
/// to get entity IDs back out of iteration, or pass one to
/// [`Commands::despawn`](crate::ecs::commands::Commands).
pub use hecs::Entity;

pub mod commands;
pub mod events;
pub mod local;
pub mod observers;
pub mod plugin;
pub mod promise;
pub mod query;
pub mod resources;
pub mod schedule;
pub mod system;
pub mod system_param;

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::ecs::{
        commands::{Commands, ResourceCommandQueue, TriggerQueue},
        local::Local,
        resources::{Read, Resources, Write},
        schedule::Schedule,
    };

    #[test]
    fn resources_insert_and_get() {
        let mut resources = Resources::default();
        resources.insert(42i32);

        assert_eq!(*resources.get::<i32>(), 42);
    }

    #[test]
    fn resources_get_mut_updates_value() {
        let mut resources = Resources::default();
        resources.insert(1i32);

        *resources.get_mut::<i32>() += 1;

        assert_eq!(*resources.get::<i32>(), 2);
    }

    #[test]
    fn resources_contains() {
        let mut resources = Resources::default();
        assert!(!resources.contains::<i32>());

        resources.insert(1i32);
        assert!(resources.contains::<i32>());
    }

    #[test]
    fn resources_remove() {
        let mut resources = Resources::default();
        resources.insert(7i32);

        let removed = resources.remove::<i32>();

        assert_eq!(removed, Some(7));
        assert!(!resources.contains::<i32>());
    }

    #[test]
    #[should_panic(expected = "Resource not found")]
    fn resources_get_missing_panics() {
        let resources = Resources::default();
        resources.get::<i32>();
    }

    #[test]
    fn schedule_runs_systems_in_order() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(0i32);
        resources.insert(hecs::CommandBuffer::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|mut counter: Write<i32>| {
            *counter += 1;
        });

        schedule.run(&mut world, &mut resources);
        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<i32>(), 2);
    }

    #[test]
    fn commands_spawn_is_applied_on_sync() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(ResourceCommandQueue::default());
        resources.insert(TriggerQueue::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|mut commands: Commands| {
            commands.spawn((1i32,));
        });

        assert_eq!(world.len(), 0);

        schedule.run(&mut world, &mut resources);

        assert_eq!(world.len(), 1);
    }

    #[test]
    fn commands_insert_resource_is_applied_on_sync() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(ResourceCommandQueue::default());
        resources.insert(TriggerQueue::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|mut commands: Commands| {
            commands.insert_resource(42i32);
        });

        assert!(!resources.contains::<i32>());

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<i32>(), 42);
    }

    #[test]
    fn commands_remove_resource_is_applied_on_sync() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(ResourceCommandQueue::default());
        resources.insert(TriggerQueue::default());
        resources.insert(42i32);

        let mut schedule = Schedule::default();
        schedule.add_system(|mut commands: Commands| {
            commands.remove_resource::<i32>();
        });

        schedule.run(&mut world, &mut resources);

        assert!(!resources.contains::<i32>());
    }

    #[test]
    fn option_read_is_some_when_resource_present() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(42i32);

        let mut schedule = Schedule::default();
        schedule.add_system(|value: Option<Read<i32>>| {
            assert_eq!(*value.expect("resource should be present"), 42);
        });

        schedule.run(&mut world, &mut resources);
    }

    #[test]
    fn option_read_is_none_when_resource_missing() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|value: Option<Read<i32>>| {
            assert!(value.is_none());
        });

        schedule.run(&mut world, &mut resources);
    }

    #[test]
    fn option_write_mutates_resource_when_present() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(1i32);

        let mut schedule = Schedule::default();
        schedule.add_system(|value: Option<Write<i32>>| {
            *value.expect("resource should be present") += 1;
        });

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<i32>(), 2);
    }

    #[test]
    fn option_write_is_none_when_resource_missing() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|value: Option<Write<i32>>| {
            assert!(value.is_none());
        });

        schedule.run(&mut world, &mut resources);
    }

    #[test]
    fn local_defaults_to_type_default() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|counter: Local<i32>| {
            assert_eq!(*counter, 0);
        });

        schedule.run(&mut world, &mut resources);
    }

    #[test]
    fn local_persists_across_runs() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let log = Rc::new(RefCell::new(Vec::new()));
        let log_clone = log.clone();

        let mut schedule = Schedule::default();
        schedule.add_system(move |mut counter: Local<i32>| {
            *counter += 1;
            log_clone.borrow_mut().push(*counter);
        });

        schedule.run(&mut world, &mut resources);
        schedule.run(&mut world, &mut resources);
        schedule.run(&mut world, &mut resources);

        assert_eq!(*log.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn local_state_is_isolated_per_system() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let log_a = Rc::new(RefCell::new(Vec::new()));
        let log_b = Rc::new(RefCell::new(Vec::new()));
        let (la, lb) = (log_a.clone(), log_b.clone());

        let mut schedule = Schedule::default();
        schedule
            .add_system(move |mut counter: Local<i32>| {
                *counter += 1;
                la.borrow_mut().push(*counter);
            })
            .add_system(move |mut counter: Local<i32>| {
                *counter += 10;
                lb.borrow_mut().push(*counter);
            });

        schedule.run(&mut world, &mut resources);
        schedule.run(&mut world, &mut resources);

        assert_eq!(*log_a.borrow(), vec![1, 2]);
        assert_eq!(*log_b.borrow(), vec![10, 20]);
    }

    fn log_a(mut log: Write<Vec<&'static str>>) {
        log.push("a");
    }

    fn log_b(mut log: Write<Vec<&'static str>>) {
        log.push("b");
    }

    #[test]
    fn before_reorders_execution_relative_to_registration_order() {
        use crate::ecs::system_param::IntoSystemConfig;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        // Registered b-then-a, but a is pinned to run before b.
        let mut schedule = Schedule::default();
        schedule.add_system(log_b).add_system(log_a.before(log_b));

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["a", "b"]);
    }

    #[test]
    fn after_resolves_against_a_system_added_later() {
        use crate::ecs::system_param::IntoSystemConfig;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        // Registered a-then-b, but a is pinned to run after b — which
        // isn't added until after the constraint is declared.
        let mut schedule = Schedule::default();
        schedule.add_system(log_a.after(log_b)).add_system(log_b);

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["b", "a"]);
    }

    #[test]
    #[should_panic(expected = "cycle")]
    fn cyclic_ordering_constraints_panic() {
        use crate::ecs::system_param::IntoSystemConfig;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        let mut schedule = Schedule::default();
        schedule
            .add_system(log_a.after(log_b))
            .add_system(log_b.after(log_a));

        schedule.run(&mut world, &mut resources);
    }

    fn log_c(mut log: Write<Vec<&'static str>>) {
        log.push("c");
    }

    #[test]
    fn higher_priority_runs_first_among_unconstrained_systems() {
        use crate::ecs::system_param::IntoSystemConfig;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        // Registered a, b, c with no after/before between them — priority
        // alone should reorder them.
        let mut schedule = Schedule::default();
        schedule
            .add_system(log_a)
            .add_system(log_b.priority(10))
            .add_system(log_c.priority(5));

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["b", "c", "a"]);
    }

    #[test]
    fn explicit_constraint_wins_over_priority() {
        use crate::ecs::system_param::IntoSystemConfig;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        // b has higher priority than a, but a is explicitly pinned before
        // b — the constraint must still win.
        let mut schedule = Schedule::default();
        schedule
            .add_system(log_b.priority(10))
            .add_system(log_a.priority(0).before(log_b));

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["a", "b"]);
    }

    #[test]
    fn chain_forces_relative_order() {
        use crate::ecs::system_param::Chain;

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        // Chained in c, a, b order — that's the order they must run in,
        // regardless of being unrelated functions otherwise.
        let mut schedule = Schedule::default();
        schedule.add_systems((log_c, log_a, log_b).chain());

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["c", "a", "b"]);
    }

    #[test]
    fn chain_after_only_constrains_its_first_system() {
        use crate::ecs::system_param::Chain;

        fn log_setup(mut log: Write<Vec<&'static str>>) {
            log.push("setup");
        }

        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(Vec::<&'static str>::new());

        let mut schedule = Schedule::default();
        schedule
            .add_systems((log_a, log_b).chain().after(log_setup))
            .add_system(log_setup);

        schedule.run(&mut world, &mut resources);

        assert_eq!(*resources.get::<Vec<&'static str>>(), vec!["setup", "a", "b"]);
    }
}
