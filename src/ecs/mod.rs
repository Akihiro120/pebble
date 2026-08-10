pub mod commands;
pub mod local;
pub mod plugin;
pub mod query;
pub mod resources;
pub mod schedule;
pub mod system;
pub mod system_param;

#[cfg(test)]
mod tests {
    use crate::ecs::{
        commands::Commands,
        resources::{Resources, Write},
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

        schedule.run(&mut world, &resources);
        schedule.run(&mut world, &resources);

        assert_eq!(*resources.get::<i32>(), 2);
    }

    #[test]
    fn commands_spawn_is_applied_on_sync() {
        let mut world = hecs::World::default();
        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());

        let mut schedule = Schedule::default();
        schedule.add_system(|mut commands: Commands| {
            commands.spawn((1i32,));
        });

        assert_eq!(world.len(), 0);

        schedule.run(&mut world, &resources);

        assert_eq!(world.len(), 1);
    }
}
