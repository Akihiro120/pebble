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
///
/// Asset sync stages ([`AssetSync`](SystemStage::AssetSync),
/// [`AssetSyncDeps`](SystemStage::AssetSyncDeps)) run **after** [`PreRender`](SystemStage::PreRender)
/// so that the GPU backend — which is delivered in `PreRender` via a one-shot
/// channel — is guaranteed to be present before asset upload is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    /// Runs once at startup, before the main loop begins.
    Startup,
    /// Runs before the main update.
    PreUpdate,
    /// Main game-logic update.
    Update,
    /// Runs after the main update.
    PostUpdate,
    /// Prepare rendering data and poll for the GPU backend.
    /// The backend resource becomes available here on the tick it finishes
    /// initialising, making it visible to the asset sync stages below.
    PreRender,
    /// Upload CPU-side source assets to the GPU backend.
    /// Runs after [`PreRender`](SystemStage::PreRender) so the backend is
    /// guaranteed to be present.
    AssetSync,
    /// Construct lazy GPU resources and upload assets that depend on other
    /// processed assets. Runs in a convergence loop so dependency chains
    /// (e.g. LazyResource A → LazyResource B) resolve within a single tick.
    AssetSyncDeps,
    /// Issue draw calls.
    Render,
    /// Cleanup or post-processing after rendering.
    PostRender,
}

impl SystemStage {
    /// Returns `true` for stages that are re-run until no new resources are
    /// inserted — collapsing multi-tick dependency chains into one tick.
    ///
    /// `Startup` is included so a deferred or async resource producer (e.g. a
    /// background thread pool result, or a [`LazyResource`](crate::assets::singleton_asset::LazyResource)-style
    /// startup system using `Option<Res<T>>` to wait) gets another pass
    /// within the same `build()` call rather than only running once.
    pub fn is_convergent(self) -> bool {
        matches!(self, Self::Startup | Self::AssetSync | Self::AssetSyncDeps)
    }
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
    /// Runtime stage order, cached once in [`build`](App::build) so
    /// [`update`](App::update) never heap-allocates per tick.
    update_stages: Vec<SystemStage>,
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
            update_stages: Vec::new(),
        }
    }

    /// Run every system in `stage` once, flush the command buffer, and return
    /// `true` if any resource was newly inserted during this pass.
    ///
    /// [`Commands::insert_resource`](crate::ecs::system::Commands::insert_resource)
    /// bumps the generation counter at queue time, so both direct inserts and
    /// deferred command-buffer inserts are detected here with no world
    /// introspection needed after the flush.
    fn run_stage_once(&mut self, stage: SystemStage) -> bool {
        let gen_before = self.resources.generation();

        if let Some(systems) = self.systems.get_mut(&stage) {
            for system in systems.iter_mut() {
                let _guard = crate::ecs::resources::set_current_system(system.name());
                system.run(&self.world, &self.resources);
            }
        }
        self.resources.get_command_buffer().run_on(&mut self.world);

        self.resources.generation() != gen_before
    }

    /// Panic before running `stage` if any of its systems declares a hard
    /// [`Res`](crate::ecs::system::Res)/[`ResMut`](crate::ecs::system::ResMut)
    /// requirement on a resource that isn't present yet.
    ///
    /// Only applied to non-convergent stages: convergent stages
    /// ([`Startup`](SystemStage::Startup), [`AssetSync`](SystemStage::AssetSync),
    /// [`AssetSyncDeps`](SystemStage::AssetSyncDeps)) are re-run precisely
    /// because a resource may not exist yet on an early pass — their systems
    /// are expected to use `Option<Res<T>>`/`Option<ResMut<T>>` to wait for
    /// it, so a hard requirement there is checked (and will panic normally)
    /// only once actually fetched.
    fn validate_stage_resources(&self, stage: SystemStage) {
        if stage.is_convergent() {
            return;
        }

        let Some(systems) = self.systems.get(&stage) else {
            return;
        };

        let mut missing: Vec<(&'static str, &'static str)> = Vec::new();
        for system in systems {
            for req in system.requires() {
                if !(req.present)(&self.world, &self.resources) {
                    missing.push((system.name(), req.name));
                    tracing::error!(
                        stage = ?stage,
                        system = system.name(),
                        resource = req.name,
                        "system requires a resource that is not yet available"
                    );
                }
            }
        }

        if !missing.is_empty() {
            missing.sort_unstable();
            missing.dedup();
            panic!(
                "{stage:?}: system(s) require resource(s) that are not yet available:\n{}\n\n\
                 Insert these via App::add_resource, or a Startup/AssetSync system, before \
                 this stage runs.",
                missing
                    .iter()
                    .map(|(system, resource)| format!(" - system `{system}` requires `{resource}`"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }

    /// Re-run `stage` until a full pass produces no new resources, up to
    /// `max_passes`. Logs a warning if the limit is reached — that usually
    /// means a [`LazyResource`](crate::assets::singleton_asset::LazyResource)
    /// or [`Asset`](crate::assets::upload::Asset) dependency is permanently
    /// unsatisfiable.
    fn run_stage_to_convergence(&mut self, stage: SystemStage, max_passes: u32) {
        for pass in 0..max_passes {
            if !self.run_stage_once(stage) {
                return;
            }
            if pass == max_passes - 1 {
                tracing::warn!(
                    "{stage:?}: convergence did not settle after {max_passes} passes — \
                     a dependency may be permanently unsatisfiable. Check for a \
                     LazyResource whose construct() or an Asset whose upload() \
                     always returns None."
                );
            }
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

        // Run startup systems to convergence (re-run until no new resources
        // appear) so a deferred/async producer — a background thread pool
        // result, a LazyResource-style construction — gets the extra passes
        // it needs within build(), then remove them from the map so they are
        // never re-run by update().
        self.run_stage_to_convergence(SystemStage::Startup, 64);
        self.systems.remove(&SystemStage::Startup);

        // For synchronous backends (headless, tests, CPU-only assets), drain
        // the asset pipeline to completion before the first frame. For windowed
        // GPU apps the backend isn't available yet so these exit immediately.
        self.run_stage_to_convergence(SystemStage::AssetSync, 64);
        self.run_stage_to_convergence(SystemStage::AssetSyncDeps, 64);

        // Cache the runtime stage order once — update() reads this slice every
        // tick without allocating.
        self.update_stages = self.systems.keys().copied().collect();

        self
    }

    /// Run all non-startup systems in stage order, flushing the command buffer
    /// after each stage. Convergent stages ([`AssetSync`](SystemStage::AssetSync),
    /// [`AssetSyncDeps`](SystemStage::AssetSyncDeps)) are re-run until no new
    /// resources are inserted, resolving dependency chains within a single tick.
    pub fn update(&mut self) {
        // Copy the stage list so the borrow on self.update_stages doesn't
        // conflict with the &mut self needed by run_stage_once / run_stage_to_convergence.
        // update_stages is a small, stable Vec (set once in build), so this clone
        // is cheap and avoids unsafe splitting borrows.
        let stages = self.update_stages.clone();
        for stage in stages {
            if stage.is_convergent() {
                self.run_stage_to_convergence(stage, 64);
            } else {
                self.validate_stage_resources(stage);
                self.run_stage_once(stage);
            }
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
