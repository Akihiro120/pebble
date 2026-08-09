use crate::{
    ecs::{
        events::{AsyncEventChannel, Events, drain_async_events},
        plugin::Plugin,
        resources::Resources,
        system::{IntoSystem, System},
        system_set::IntoSystemSet,
    },
};
use std::collections::{BTreeMap, BinaryHeap, HashMap};

/// Determines when during a frame a system is executed.
///
/// [`Startup`](SystemStage::Startup) runs on every tick until a system on it
/// returns `Some(())` (then never again). Systems returning `Option<()>` are
/// automatically "once" — they retry each tick until they succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    /// Run-once startup systems — any system added here that returns
    /// `Option<()>` runs until it returns `Some(())`, then is permanently
    /// retired.
    Startup,
    /// Upload CPU-side source assets to the GPU backend.
    AssetSync,
    /// Construct GPU resources and upload assets that depend on other
    /// processed assets.
    AssetSyncDeps,
    /// Before the main update.
    PreUpdate,
    /// Main game-logic update.
    Update,
    /// After the main update.
    PostUpdate,
    /// Prepare rendering data and poll for the GPU backend.
    PreRender,
    /// Issue draw calls.
    Render,
    /// Cleanup or post-processing after rendering.
    PostRender,
}

/// Fixed per-tick order for all stages.
const ALL_STAGES: [SystemStage; 9] = [
    SystemStage::Startup,
    SystemStage::AssetSync,
    SystemStage::AssetSyncDeps,
    SystemStage::PreUpdate,
    SystemStage::Update,
    SystemStage::PostUpdate,
    SystemStage::PreRender,
    SystemStage::Render,
    SystemStage::PostRender,
];

/// Callback used to drive the application's main loop.
///
/// Set with [`App::set_runner`]. The default runner calls [`App::update`] in
/// an infinite loop.
pub type AppRunner = Box<dyn FnOnce(App)>;

/// The central application object.
///
/// `App` owns the ECS world, resources, plugins, and systems. The typical
/// lifecycle is:
///
/// 1. Create with [`App::new`].
/// 2. Register plugins with [`add_plugin`](App::add_plugin).
/// 3. Call [`build`](App::build) to run all plugin registrations and sort systems.
/// 4. Call [`run`](App::run) to hand control to the runner.
pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: BTreeMap<SystemStage, Vec<Box<dyn System>>>,
    runner: Option<AppRunner>,
    /// One closure per event type registered via [`add_event`](App::add_event),
    /// each calling that type's [`Events::update`] to age its buffers. Run
    /// at the front of every [`update`](App::update) tick, before any user
    /// system, so a reader anywhere in the tick sees a consistent view.
    event_updaters: Vec<Box<dyn FnMut(&hecs::World, &Resources)>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new `App` with an empty world and a default infinite-loop runner.
    ///
    /// Builds in [`TimePlugin`](crate::time::TimePlugin) — `Res<Time>` works
    /// without registering anything yourself — along with
    /// [`GamepadPlugin`](crate::gamepad::GamepadPlugin) (`Res<Gamepads>`) and
    /// [`AudioPlugin`](crate::audio::AudioPlugin) (`Res<AudioOutput>`).
    pub fn new() -> Self {
        let mut world = hecs::World::default();
        let mut resources = Resources::new(&mut world);
        resources.insert_resource(&mut world, ());

        let mut app = Self {
            world: world,
            resources: resources,
            plugins: Vec::new(),
            systems: BTreeMap::new(),
            runner: Some(Box::new(|mut app| {
                loop {
                    app.update();
                }
            })),
            event_updaters: Vec::new(),
        };
        app.add_plugin(crate::time::TimePlugin);
        app.add_plugin(crate::gamepad::GamepadPlugin);
        app.add_plugin(crate::audio::AudioPlugin);
        app
    }

    /// Run every system in `stage` once, then flush the command buffer.
    fn run_stage_once(&mut self, stage: SystemStage) {
        if let Some(systems) = self.systems.get_mut(&stage) {
            for system in systems.iter_mut() {
                let _guard = crate::ecs::resources::set_current_system(system.name());
                system.run(&self.world, &self.resources);
            }
        }
        self.resources.get_command_buffer().run_on(&mut self.world);
    }

    /// Queue a plugin to be built during [`build`](App::build).
    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Insert a resource into the world immediately.
    pub fn add_resource(&mut self, res: impl hecs::Component) -> &mut Self {
        self.resources.insert_resource(&mut self.world, res);
        self
    }

    /// Borrow resource `T`, panicking if it is absent.
    pub fn get_resource<'a, T: hecs::Component>(&'a self) -> hecs::Ref<'a, T> {
        self.resources.get_resource(&self.world)
    }

    /// Mutably borrow resource `T`, panicking if it is absent.
    pub fn get_resource_mut<'a, T: hecs::Component>(&'a self) -> hecs::RefMut<'a, T> {
        self.resources.get_resource_mut(&self.world)
    }

    /// Insert resource `T` only if it is not already present.
    ///
    /// Returns `true` if the resource was inserted.
    pub fn try_insert_resource<T: hecs::Component>(&mut self, res: T) -> bool {
        self.resources.try_insert(&mut self.world, res)
    }

    /// Register event type `T`, making [`EventWriter<T>`](crate::ecs::events::EventWriter)
    /// and [`EventReader<T>`](crate::ecs::events::EventReader) usable as
    /// system parameters.
    pub fn add_event<T: hecs::Component>(&mut self) -> &mut Self {
        self.try_insert_resource(Events::<T>::default());
        self.event_updaters.push(Box::new(|world, resources| {
            resources.get_resource_mut::<Events<T>>(world).update();
        }));
        self
    }

    /// Register event type `T` as in [`add_event`](Self::add_event), and
    /// additionally make [`AsyncEventWriter<T>`](crate::ecs::events::AsyncEventWriter)
    /// usable as a system parameter.
    pub fn add_async_event<T: hecs::Component>(&mut self) -> &mut Self {
        self.add_event::<T>();
        self.try_insert_resource(AsyncEventChannel::<T>::new());
        self.add_system(SystemStage::PreUpdate, drain_async_events::<T>);
        self
    }

    /// Register a single system to run at `stage`.
    pub fn add_system<Marker>(
        &mut self,
        stage: SystemStage,
        system: impl IntoSystem<Marker> + 'static,
    ) -> &mut Self {
        self.systems
            .entry(stage)
            .or_default()
            .push(Box::new(system.into_system()));
        self
    }

    /// Register multiple systems to run at `stage`.
    ///
    /// Accepts a tuple of systems via [`IntoSystemSet`].
    pub fn add_systems<Marker>(
        &mut self,
        stage: SystemStage,
        systems: impl IntoSystemSet<Marker>,
    ) -> &mut Self {
        let entry = self.systems.entry(stage).or_default();
        entry.extend(systems.into_system_set());
        self
    }

    /// Topologically sort `systems` by each system's [`System::after_ids`]/[`System::before_ids`]
    /// constraints, breaking ties by original registration order.
    ///
    /// Panics if the constraints form a cycle.
    fn sort_stage(stage: SystemStage, systems: &mut Vec<Box<dyn System>>) {
        let id_index: HashMap<std::any::TypeId, usize> = systems
            .iter()
            .enumerate()
            .map(|(i, s)| (s.ordering_id(), i))
            .collect();

        let n = systems.len();
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree = vec![0usize; n];

        for (i, system) in systems.iter().enumerate() {
            for id in system.after_ids() {
                if let Some(&dep) = id_index.get(id) {
                    adjacency[dep].push(i);
                    in_degree[i] += 1;
                }
            }
            for id in system.before_ids() {
                if let Some(&dependent) = id_index.get(id) {
                    adjacency[i].push(dependent);
                    in_degree[dependent] += 1;
                }
            }
        }

        // Min-heap on original index so ties resolve to registration order.
        let mut ready: BinaryHeap<std::cmp::Reverse<usize>> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == 0)
            .map(|(i, _)| std::cmp::Reverse(i))
            .collect();

        let mut order = Vec::with_capacity(n);
        while let Some(std::cmp::Reverse(u)) = ready.pop() {
            order.push(u);
            for &v in &adjacency[u] {
                in_degree[v] -= 1;
                if in_degree[v] == 0 {
                    ready.push(std::cmp::Reverse(v));
                }
            }
        }

        if order.len() != n {
            let stuck: Vec<&'static str> = (0..n)
                .filter(|i| in_degree[*i] > 0)
                .map(|i| systems[i].name())
                .collect();
            panic!(
                "{stage:?}: system ordering constraints form a cycle among: {stuck:?}"
            );
        }

        let mut taken: Vec<Option<Box<dyn System>>> = systems.drain(..).map(Some).collect();
        for i in order {
            systems.push(taken[i].take().unwrap());
        }
    }

    /// Build all plugins and sort systems.
    ///
    /// Plugins may register additional plugins during their `build` call; this
    /// repeats until no new plugins are added, up to a hard limit of 64 passes
    /// to catch accidental infinite registration cycles.
    pub fn build(&mut self) -> &mut Self {
        let mut iterations = 0;
        const MAX_PLUGIN_BUILD_ITERATIONS: u32 = 64;

        while !self.plugins.is_empty() {
            iterations += 1;
            if iterations > MAX_PLUGIN_BUILD_ITERATIONS {
                panic!(
                    "App::build() exceeded {MAX_PLUGIN_BUILD_ITERATIONS} plugin-registration passes — \
                 likely a cycle where plugins keep registering each other. Check for a plugin whose \
                 build() unconditionally re-adds itself or another plugin that re-adds it."
                );
            }
            let plugins: Vec<_> = self.plugins.drain(..).collect();
            for plugin in plugins {
                plugin.build(self);
            }
        }

        for (stage, systems) in self.systems.iter_mut() {
            Self::sort_stage(*stage, systems);
        }

        self
    }

    /// Run every stage once per tick, in [`ALL_STAGES`] order.
    pub fn update(&mut self) {
        for updater in self.event_updaters.iter_mut() {
            updater(&self.world, &self.resources);
        }
        for stage in ALL_STAGES {
            self.run_stage_once(stage);
        }
    }

    /// Replace the default runner with a custom one.
    ///
    /// The runner receives ownership of the `App` and is responsible for
    /// calling [`update`](App::update) at the appropriate cadence (e.g. driven
    /// by a window event loop).
    pub fn set_runner<F>(&mut self, runner: F) -> &mut Self
    where
        F: FnOnce(App) + 'static,
    {
        self.runner = Some(Box::new(runner));
        self
    }

    /// Consume the app and hand it to the configured runner.
    ///
    /// Panics if no runner has been set.
    pub fn run(&mut self) {
        let mut owned_app = std::mem::take(self);
        let runner = owned_app.runner.take().expect("No runner found!");
        runner(owned_app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::system::{ResMut, SystemOrderingExt};

    struct Order(Vec<&'static str>);

    fn sys_a(mut o: ResMut<Order>) {
        o.0.push("a");
    }
    fn sys_b(mut o: ResMut<Order>) {
        o.0.push("b");
    }
    fn sys_c(mut o: ResMut<Order>) {
        o.0.push("c");
    }

    #[test]
    fn systems_run_in_declared_order() {
        let mut app = App::new();
        app.add_resource(Order(Vec::new()));

        // Registered in a-b-c order, but both a and b declare they must run
        // after c — the sort should move c first while leaving a before b
        // (their relative registration order) intact.
        app.add_system(SystemStage::Update, sys_a.after(sys_c));
        app.add_system(SystemStage::Update, sys_b.after(sys_c));
        app.add_system(SystemStage::Update, sys_c);

        app.build();
        app.update();

        let order = app.get_resource::<Order>();
        assert_eq!(order.0, vec!["c", "a", "b"]);
    }

    #[test]
    #[should_panic(expected = "cycle")]
    fn cyclic_ordering_constraints_panic() {
        let mut app = App::new();
        app.add_resource(Order(Vec::new()));

        app.add_system(SystemStage::Update, sys_a.after(sys_b));
        app.add_system(SystemStage::Update, sys_b.after(sys_a));

        app.build();
    }

    #[test]
    fn time_plugin_is_already_built_into_a_fresh_app() {
        let mut app = App::new();
        app.build();

        // Doesn't panic — `Time` exists without anyone calling
        // `add_plugin(TimePlugin)` themselves.
        let _ = app.get_resource::<crate::time::Time>();
    }

    #[test]
    fn registering_time_plugin_again_does_not_double_register_its_system() {
        // Baseline: however many PreUpdate systems App::new()'s own
        // automatic plugins (Time, Gamepad, ...) register on their own —
        // not hardcoded, so this stays correct as more get added.
        let mut baseline = App::new();
        baseline.build();
        let baseline_count = baseline.systems.get(&SystemStage::PreUpdate).map_or(0, Vec::len);

        let mut app = App::new();
        app.add_plugin(crate::time::TimePlugin); // redundant - App::new() already built it in
        app.build();

        // If TimePlugin weren't idempotent, this would be one more than baseline.
        let tick_systems = app.systems.get(&SystemStage::PreUpdate).map_or(0, Vec::len);
        assert_eq!(tick_systems, baseline_count);
    }
}
