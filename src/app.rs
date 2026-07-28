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
/// [`Startup`](SystemStage::Startup) systems each run at most once — but not
/// necessarily during [`App::build`]: a system whose hard [`Res`](crate::ecs::system::Res)/[`ResMut`](crate::ecs::system::ResMut)
/// requirements aren't satisfied yet is skipped (never panicked) and retried
/// every tick, prioritized ahead of [`PreUpdate`](SystemStage::PreUpdate)
/// and everything after it, until it fires exactly once. This lets a
/// `Startup` system depend on something that only becomes real several
/// frames in — a `LazyResource` built from an async GPU backend, for
/// example — without moving it off `Startup`.
///
/// [`AssetSync`](SystemStage::AssetSync)/[`AssetSyncDeps`](SystemStage::AssetSyncDeps)
/// are similarly prioritized: they (and any newly-ready `Startup` systems)
/// are re-run to convergence at the front of every tick and again after
/// every other stage, so newly queued asset/resource work is drained before
/// gameplay stages continue rather than waiting for the next tick's front
/// pass. All other stages run once per [`App::update`] tick, in the order
/// declared below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    /// Each system runs at most once, as soon as its requirements are met —
    /// possibly during [`App::build`], possibly several ticks into
    /// [`App::update`]. See the type-level docs above.
    Startup,
    /// Before the main update.
    PreUpdate,
    /// Main game-logic update.
    Update,
    /// After the main update.
    PostUpdate,
    /// Prepare rendering data and poll for the GPU backend.
    /// The backend resource becomes available here on the tick it finishes
    /// initialising, making it visible to the asset sync stages.
    PreRender,
    /// Upload CPU-side source assets to the GPU backend.
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
    /// Returns `true` for stages that are prioritized and re-run until a
    /// full pass produces no new resources, instead of running once in
    /// their declared position in the tick order. See the type-level docs
    /// on [`SystemStage`].
    pub fn is_convergent(self) -> bool {
        matches!(self, Self::Startup | Self::AssetSync | Self::AssetSyncDeps)
    }
}

/// Fixed per-tick order for every stage *except* the convergent ones
/// (`Startup`, `AssetSync`, `AssetSyncDeps`), which are driven separately by
/// [`App::reconverge`] — at the front of the tick and again after each of
/// these — rather than appearing in this list.
const TICK_STAGES: [SystemStage; 6] = [
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
    /// Per-`Startup`-system "has it run yet" flags, indexed the same as
    /// `systems[&SystemStage::Startup]`. Sized once in [`build`](App::build).
    /// A system is only ever invoked once its [`System::requires`] are all
    /// satisfied, and once invoked it is never invoked again.
    startup_done: Vec<bool>,
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
            startup_done: Vec::new(),
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

    /// Run every not-yet-fired `Startup` system whose hard requirements are
    /// currently satisfied, marking each one done so it never runs again.
    /// A system whose requirements aren't met yet is left pending — silently,
    /// no panic — for another attempt on a later pass/tick. Returns `true` if
    /// any system ran (i.e. a resource may have changed).
    fn run_startup_ready(&mut self) -> bool {
        let Some(systems) = self.systems.get_mut(&SystemStage::Startup) else {
            return false;
        };
        if self.startup_done.len() != systems.len() {
            self.startup_done.resize(systems.len(), false);
        }

        let mut any_ran = false;
        for (idx, system) in systems.iter_mut().enumerate() {
            if self.startup_done[idx] {
                continue;
            }
            let ready = system
                .requires()
                .iter()
                .all(|req| (req.present)(&self.world, &self.resources));
            if !ready {
                continue;
            }

            let _guard = crate::ecs::resources::set_current_system(system.name());
            system.run(&self.world, &self.resources);
            self.startup_done[idx] = true;
            any_ran = true;
        }

        if any_ran {
            self.resources.get_command_buffer().run_on(&mut self.world);
        }
        any_ran
    }

    /// Panic before running `stage` if any of its systems declares a hard
    /// [`Res`](crate::ecs::system::Res)/[`ResMut`](crate::ecs::system::ResMut)
    /// requirement on a resource that isn't present yet.
    ///
    /// Only applied to non-convergent stages: convergent stages
    /// ([`Startup`](SystemStage::Startup), [`AssetSync`](SystemStage::AssetSync),
    /// [`AssetSyncDeps`](SystemStage::AssetSyncDeps)) are handled by
    /// [`reconverge`](App::reconverge) instead, which skips systems that
    /// aren't ready rather than treating it as an error.
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

    /// Prioritize resource/asset construction: run any not-yet-fired
    /// `Startup` systems that are now ready, then `AssetSync`, then
    /// `AssetSyncDeps` — repeating the whole trio until a full pass produces
    /// no new resources, up to `max_passes`. `Startup` runs first each pass
    /// so a same-pass consumer (an `AssetSyncDeps` system, say) can see what
    /// it just produced. Logs a warning if the limit is reached — that
    /// usually means a [`LazyResource`](crate::assets::singleton_asset::LazyResource)
    /// or [`Asset`](crate::assets::upload::Asset) dependency is permanently
    /// unsatisfiable (or a `Startup` system's requirement is never met).
    ///
    /// Called at the front of every tick and again after every stage in
    /// [`update`](App::update) (and once during [`build`](App::build)), so
    /// newly-queued asset/resource work — and any `Startup` system it
    /// unblocks — is drained immediately instead of waiting for next tick's
    /// front pass.
    fn reconverge(&mut self, max_passes: u32) {
        for pass in 0..max_passes {
            let gen_before = self.resources.generation();

            self.run_startup_ready();
            self.run_stage_once(SystemStage::AssetSync);
            self.run_stage_once(SystemStage::AssetSyncDeps);

            if self.resources.generation() == gen_before {
                return;
            }
            if pass == max_passes - 1 {
                tracing::warn!(
                    "Startup/AssetSync/AssetSyncDeps did not settle after {max_passes} passes — \
                     a dependency may be permanently unsatisfiable. Check for a Startup system \
                     whose Res/ResMut requirement is never met, a LazyResource whose construct() \
                     always returns None, or an Asset whose upload() always returns None."
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

        // Size the per-system done-tracking now that every plugin has
        // registered its Startup systems.
        let startup_len = self
            .systems
            .get(&SystemStage::Startup)
            .map(Vec::len)
            .unwrap_or(0);
        self.startup_done = vec![false; startup_len];

        // Resolve as much as possible synchronously (headless/CPU-only
        // backends, tests) so resources are ready immediately after
        // build(). Anything still pending — a Startup system waiting on an
        // async GPU backend, say — keeps getting retried every tick by
        // update(), prioritized ahead of PreUpdate/Update/etc.
        self.reconverge(64);

        self
    }

    /// Run every stage once per tick, in [`TICK_STAGES`] order. Before every
    /// tick, and again after every stage, [`reconverge`](App::reconverge)
    /// drains `Startup`/`AssetSync`/`AssetSyncDeps` — so newly-queued asset
    /// or resource work (and any `Startup` system it unblocks) is handled
    /// immediately rather than waiting for the next tick's front pass.
    pub fn update(&mut self) {
        self.reconverge(64);

        for stage in TICK_STAGES {
            self.validate_stage_resources(stage);
            self.run_stage_once(stage);
            self.reconverge(64);
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
