use crate::{
    assets::required::RequiredResources,
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
/// There's no dedicated "run once at startup" stage — instead, any system on
/// any stage can be made to run at most once with [`.once()`](crate::ecs::system::OnceExt::once),
/// which turns "have I already done this" into the function's own return
/// value (`Some(())` = done, retire; `None` = not ready, try again next
/// tick) instead of a special stage with its own rules. A `.once()` system
/// naturally waits as many ticks as it needs to (an async GPU backend, a
/// `LazyResource` that isn't built yet) using the exact same requirement
/// checks as every other system on its stage.
///
/// [`AssetSync`](SystemStage::AssetSync)/[`AssetSyncDeps`](SystemStage::AssetSyncDeps)
/// are prioritized: they're re-run to convergence (repeated until a full
/// pass produces no new resources) at the front of every tick and again
/// after every other stage, so newly queued asset/resource work is drained
/// before gameplay stages continue rather than waiting for the next tick's
/// front pass. All other stages run once per [`App::update`] tick, in the
/// order declared below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
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
        matches!(self, Self::AssetSync | Self::AssetSyncDeps)
    }
}

/// Fixed per-tick order for every stage *except* the convergent ones
/// (`AssetSync`, `AssetSyncDeps`), which are driven separately by
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

/// Whether a system is safe to run right now, given its declared
/// [`System::requires`]. See [`App::check_readiness`].
enum Readiness {
    /// No unmet requirement — go ahead and run it.
    Ready,
    /// Missing a resource that some plugin has declared (via
    /// [`RequiredResources::provides`]) it eventually provides — wait
    /// quietly, no error, and try again next pass/tick.
    WaitingOnLazy,
    /// Missing a resource nothing has ever declared it will provide —
    /// almost certainly a genuine oversight, not a timing issue.
    MissingUnprovided {
        system: &'static str,
        resource: &'static str,
        hint: Option<&'static str>,
    },
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
///    validate required resources, and settle `AssetSync`/`AssetSyncDeps`
///    as far as they can go synchronously.
/// 4. Call [`run`](App::run) to hand control to the runner.
pub struct App {
    pub(crate) world: hecs::World,
    pub(crate) resources: Resources,
    plugins: Vec<Box<dyn Plugin>>,
    systems: BTreeMap<SystemStage, Vec<Box<dyn System>>>,
    runner: Option<AppRunner>,
    pub(crate) required: RequiredResources,
    /// One closure per event type registered via [`add_event`](App::add_event),
    /// each calling that type's [`Events::update`] to age its buffers. Run
    /// at the front of every [`update`](App::update) tick, before any user
    /// system, so a reader anywhere in the tick sees a consistent view. Kept
    /// here rather than as regular systems because they must run before
    /// every stage, not just one, and ordering that generically against
    /// arbitrary user systems isn't worth the complexity.
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
    /// without registering anything yourself.
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
            required: RequiredResources::new(),
            event_updaters: Vec::new(),
        };
        app.add_plugin(crate::time::TimePlugin);
        app
    }

    /// Check `system` against `required` without running it. See
    /// [`Readiness`]. Used by [`run_stage_once`](App::run_stage_once) for
    /// every stage.
    ///
    /// A free function (rather than a `&self` method) so it only borrows
    /// `world`/`resources`/`required` — the specific fields still available
    /// while a caller holds a `&mut` borrow of `self.systems` to iterate the
    /// very system being checked.
    fn check_readiness(
        world: &hecs::World,
        resources: &Resources,
        required: &RequiredResources,
        system: &dyn System,
    ) -> Readiness {
        for req in system.requires() {
            if (req.present)(world, resources) {
                continue;
            }
            if required.is_provided(req.type_id) {
                return Readiness::WaitingOnLazy;
            }
            return Readiness::MissingUnprovided {
                system: system.name(),
                resource: req.name,
                hint: req.hint,
            };
        }
        Readiness::Ready
    }

    /// The advice appended to a "missing resource" panic when the
    /// [`RequiredResource`](crate::ecs::system::RequiredResource) didn't
    /// supply its own more specific `hint` — the generic fallback,
    /// appropriate for a plain `Res<T>`/`ResMut<T>` on an arbitrary
    /// resource type with no dedicated registration method of its own.
    fn generic_missing_resource_hint(resource: &'static str) -> String {
        format!(
            "If `{resource}` genuinely arrives later (an async backend, a LazyResource, \
             an Asset upload), call `app.required.provides::<{resource}>()` in whichever \
             plugin inserts it, and this will wait instead of erroring. Otherwise, insert \
             it via App::add_resource before this stage runs."
        )
    }

    /// Panic with a message naming both the offending system and resource,
    /// plus either its param-specific `hint` (e.g. "call `app.add_event::<T>()`")
    /// or, absent that, the generic fallback advice.
    fn panic_missing_unprovided(
        stage: SystemStage,
        system: &'static str,
        resource: &'static str,
        hint: Option<&'static str>,
    ) -> ! {
        let advice = hint
            .map(str::to_string)
            .unwrap_or_else(|| Self::generic_missing_resource_hint(resource));
        panic!(
            "{stage:?}: system `{system}` requires `{resource}`, which nothing has \
             registered as provided.\n\n{advice}"
        );
    }

    /// Pre-flight check, run once at the end of [`build`](Self::build): walk
    /// every registered system in every stage and evaluate its
    /// [`System::requires`] via [`check_readiness`](Self::check_readiness),
    /// the same logic [`run_stage_once`](Self::run_stage_once) applies lazily
    /// as each stage actually runs. A system waiting on a resource that
    /// something else has [declared it provides](RequiredResources::provides)
    /// is left alone — it'll show up once that plugin's async/lazy work
    /// settles. A system requiring a resource that *nothing* provides and
    /// that isn't already present is a genuine configuration mistake, and
    /// every such mistake across the whole app is collected into one panic
    /// here — instead of each one surfacing separately, one at a time, the
    /// first time its particular stage happens to run.
    fn validate_requirements(&self) {
        let mut missing = Vec::new();

        for (stage, systems) in self.systems.iter() {
            for system in systems.iter() {
                if let Readiness::MissingUnprovided { system, resource, hint } =
                    Self::check_readiness(&self.world, &self.resources, &self.required, system.as_ref())
                {
                    missing.push((*stage, system, resource, hint));
                }
            }
        }

        if missing.is_empty() {
            return;
        }

        let mut message = String::from(
            "Pebble startup validation failed — the following systems require resources \
             that nothing has registered as provided:\n",
        );
        for (stage, system, resource, hint) in &missing {
            let advice = hint
                .map(str::to_string)
                .unwrap_or_else(|| Self::generic_missing_resource_hint(resource));
            message.push_str(&format!("\n{stage:?}: system `{system}` requires `{resource}`\n  {advice}\n"));
        }
        panic!("{message}");
    }

    /// Run every system in `stage` once, flush the command buffer, and return
    /// `true` if any resource was newly inserted during this pass.
    ///
    /// A system with an unmet hard [`Res`](crate::ecs::system::Res)/[`ResMut`](crate::ecs::system::ResMut)
    /// requirement is skipped for this pass if the resource is registered as
    /// [provided](RequiredResources::provides) somewhere (it'll get there —
    /// just not yet), or panics immediately, naming the system and resource,
    /// if nothing ever declared it would provide that resource at all.
    ///
    /// [`Commands::insert_resource`](crate::ecs::system::Commands::insert_resource)
    /// bumps the generation counter at queue time, so both direct inserts and
    /// deferred command-buffer inserts are detected here with no world
    /// introspection needed after the flush.
    fn run_stage_once(&mut self, stage: SystemStage) -> bool {
        let gen_before = self.resources.generation();

        if let Some(systems) = self.systems.get_mut(&stage) {
            for system in systems.iter_mut() {
                match Self::check_readiness(&self.world, &self.resources, &self.required, system.as_ref()) {
                    Readiness::Ready => {}
                    Readiness::WaitingOnLazy => continue,
                    Readiness::MissingUnprovided { system, resource, hint } => {
                        Self::panic_missing_unprovided(stage, system, resource, hint)
                    }
                }
                let _guard = crate::ecs::resources::set_current_system(system.name());
                system.run(&self.world, &self.resources);
            }
        }
        self.resources.get_command_buffer().run_on(&mut self.world);

        self.resources.generation() != gen_before
    }

    /// Run `AssetSync`, then `AssetSyncDeps`, repeating both until a full
    /// pass produces no new resources, up to `max_passes`. Logs a warning if
    /// the limit is reached — that usually means a [`LazyResource`](crate::assets::singleton_asset::LazyResource)
    /// whose `construct()` or an [`Asset`](crate::assets::upload::Asset)
    /// whose `upload()` always returns `None`.
    ///
    /// Called at the front of every tick and again after every stage in
    /// [`update`](App::update) (and once during [`build`](App::build)), so
    /// newly-queued asset/resource work is drained immediately instead of
    /// waiting for the next tick's front pass.
    fn reconverge(&mut self, max_passes: u32) {
        for pass in 0..max_passes {
            let gen_before = self.resources.generation();

            self.run_stage_once(SystemStage::AssetSync);
            self.run_stage_once(SystemStage::AssetSyncDeps);

            if self.resources.generation() == gen_before {
                return;
            }
            if pass == max_passes - 1 {
                tracing::warn!(
                    "AssetSync/AssetSyncDeps did not settle after {max_passes} passes — a \
                     dependency may be permanently unsatisfiable. Check for a LazyResource \
                     whose construct() or an Asset whose upload() always returns None."
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

    /// Declare that resource type `T` is expected to be inserted later —
    /// possibly asynchronously (a background thread's result, a hand-rolled
    /// lazy resource) rather than up front. A system elsewhere with a hard
    /// `Res<T>`/`ResMut<T>` requirement on `T` will then wait quietly for it
    /// instead of `App` treating the absence as a configuration mistake and
    /// panicking.
    ///
    /// [`GraphicsPlugin`](crate::rendering::graphics_plugin::GraphicsPlugin)
    /// and [`LazyResourcePlugin`](crate::assets::singleton_asset::LazyResourcePlugin)
    /// already call this for the backend and lazy resource types they
    /// manage — reach for this directly only for your own resource types
    /// that arrive outside of those.
    pub fn provides<T: 'static>(&mut self) -> &mut Self {
        self.required.provides::<T>();
        self
    }

    /// Register event type `T`, making [`EventWriter<T>`](crate::ecs::events::EventWriter)
    /// and [`EventReader<T>`](crate::ecs::events::EventReader) usable as
    /// system parameters.
    ///
    /// Inserts the backing [`Events<T>`] resource (a no-op if `T` was
    /// already registered) and schedules its per-tick aging, which is what
    /// gives events sent during tick `N` a consistent two-tick lifetime —
    /// visible for the rest of `N` and all of `N + 1` — regardless of which
    /// stage the writer or reader runs in.
    pub fn add_event<T: hecs::Component>(&mut self) -> &mut Self {
        self.try_insert_resource(Events::<T>::default());
        self.event_updaters.push(Box::new(|world, resources| {
            resources.get_resource_mut::<Events<T>>(world).update();
        }));
        self
    }

    /// Register event type `T` as in [`add_event`](Self::add_event), and
    /// additionally make [`AsyncEventWriter<T>`](crate::ecs::events::AsyncEventWriter)
    /// usable as a system parameter — the friendly way to turn a background
    /// task's result into a `T` event once it resolves, instead of hand-
    /// rolling a pending-task resource and poll system yourself.
    ///
    /// Requires [`BackgroundTasksPlugin`](crate::threading::BackgroundTasksPlugin)
    /// to be registered before any system using `AsyncEventWriter<T>` runs —
    /// that's what [`AsyncEventWriter::spawn`](crate::ecs::events::AsyncEventWriter::spawn)
    /// drives the future through.
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
    /// constraints (referencing other systems' [`System::ordering_id`] within
    /// the same stage), breaking ties by original registration order.
    ///
    /// Panics if the constraints form a cycle, naming every system still
    /// stuck once no more zero-dependency systems remain.
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

    /// Build all plugins and validate required resources.
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

        // Resolve as much as possible synchronously (headless/CPU-only
        // backends, tests) so resources are ready immediately after
        // build(). Anything still pending (an async GPU backend, say)
        // keeps getting retried every tick by update().
        self.reconverge(64);

        self.validate_requirements();

        self
    }

    /// Run every stage once per tick, in [`TICK_STAGES`] order. Before every
    /// tick, and again after every stage, [`reconverge`](App::reconverge)
    /// drains `AssetSync`/`AssetSyncDeps` — so newly-queued asset or
    /// resource work is handled immediately rather than waiting for the
    /// next tick's front pass.
    pub fn update(&mut self) {
        for updater in self.event_updaters.iter_mut() {
            updater(&self.world, &self.resources);
        }

        self.reconverge(64);

        for stage in TICK_STAGES {
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
        let mut app = App::new();
        app.add_plugin(crate::time::TimePlugin); // redundant - App::new() already built it in
        app.build();

        // A fresh App's PreUpdate stage contains only Time's own tick
        // system; if TimePlugin weren't idempotent, this would be 2.
        let tick_systems = app.systems.get(&SystemStage::PreUpdate).map_or(0, Vec::len);
        assert_eq!(tick_systems, 1);
    }
}
