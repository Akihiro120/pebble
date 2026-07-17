use crate::{
    assets::required::RequiredResources,
    ecs::{
        plugin::Plugin,
        resources::Resources,
        system::{IntoSystem, System},
        system_set::IntoSystemSet,
    },
};
use std::collections::BTreeMap;

/// Determines when during a frame a system is executed.
///
/// Stages are iterated in the order defined here — [`Startup`](SystemStage::Startup)
/// runs once during [`App::build`], all others run every [`App::update`] tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    /// Runs once at startup, before the main loop begins.
    Startup,
    /// Upload CPU assets to the GPU backend.
    AssetSync,
    /// Upload assets that depend on other GPU assets.
    AssetSyncDeps,
    /// Runs before the main update.
    PreUpdate,
    /// Main game-logic update.
    Update,
    /// Runs after the main update.
    PostUpdate,
    /// Prepare rendering data before the render stage.
    PreRender,
    /// Issue draw calls.
    Render,
    /// Cleanup or post-processing after rendering.
    PostRender,
}

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
/// 3. Call [`build`](App::build) to run all plugin registrations, execute
///    startup systems, and validate required resources.
/// 4. Call [`run`](App::run) to hand control to the runner.
pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: BTreeMap<SystemStage, Vec<Box<dyn System>>>,
    runner: Option<AppRunner>,
    pub(crate) required: RequiredResources,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new `App` with an empty world and a default infinite-loop runner.
    pub fn new() -> Self {
        let mut world = hecs::World::default();
        let mut resources = Resources::new(&mut world);
        resources.insert_resource(&mut world, ());

        Self {
            world: world,
            resources: resources,
            plugins: Vec::new(),
            systems: BTreeMap::new(),
            runner: Some(Box::new(|mut app| {
                loop {
                    app.update();
                }
            })),
            required: RequiredResources::new(),
        }
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

    /// Build all plugins, run startup systems, and validate required resources.
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

        self.required.validate();

        if let Some(systems) = self.systems.remove(&SystemStage::Startup) {
            for mut system in systems {
                system.run(&self.world, &self.resources);
            }
            self.resources.get_command_buffer().run_on(&mut self.world);
        }
        self
    }

    /// Run all non-startup systems in stage order, then flush the command buffer.
    pub fn update(&mut self) {
        for systems in self.systems.values_mut() {
            for system in systems {
                system.run(&self.world, &self.resources);
            }
            self.resources.get_command_buffer().run_on(&mut self.world);
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
