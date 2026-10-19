use std::collections::BTreeMap;

use crate::ecs::{
    commands::{ResourceCommandQueue, TriggerQueue},
    events::{Events, age_events},
    observers::{IntoObserverSystem, Observers},
    plugin::Plugin,
    resources::Resources,
    schedule::Schedule,
    stream::{Emitter, Stream, StreamState},
    system::SystemStage,
    system_param::{IntoSystem, SystemChain, SystemConfig},
};

/// Set this to `true` (e.g. `commands.insert_resource(AppExit(true))`) to
/// stop the app after the current tick. Both the default headless loop and
/// the windowing plugin's runner check it after every tick, whether or not the
/// GPU backend ever came up — a failed acquisition sets it.
#[derive(Default)]
pub struct AppExit(pub bool);

/// Whether the GPU backend has finished initializing. `false` until a
/// plugin (e.g. `GraphicsPlugin`) acquires one and flips it — until then,
/// [`App::update`] only runs `gpu_schedules`, not the regular stages.
#[derive(Default)]
pub struct BackendReady(pub bool);

/// A boxed, type-erased delivery of one externally-sent event — the closure
/// captures the concrete `E`, so no type registry is needed.
type Delivery = Box<dyn FnOnce(&Resources) + Send>;

/// A cloneable, thread-safe handle for sending events into a running app
/// from outside the ECS — the pebble equivalent of winit's
/// `EventLoopProxy`. Get one with [`App::handle`] before [`App::run`]
/// consumes the app; clones can live on background threads, in async
/// tasks, or in browser callbacks.
///
/// Events land at the start of the next tick, in the same double-buffered
/// [`Events<T>`] queues an [`EventWriter`](crate::ecs::events::EventWriter)
/// feeds — readers can't tell the difference. The event type must be
/// registered via [`App::add_event`]; an unregistered type is dropped with
/// a warning, never a panic.
///
/// ```ignore
/// let app = App::new().add_event::<PoseUpdate>() /* ... */;
/// let handle = app.handle();
/// websocket.on_message(move |msg| handle.send(PoseUpdate::from(msg)));
/// app.run();
/// ```
// Deliberately not Default: a handle only makes sense wired to an app, and
// its inner Emitter comes from the Stream pair App::default creates.
#[derive(Clone)]
pub struct AppHandle {
    tx: Emitter<Delivery>,
}

impl AppHandle {
    /// Queues `event` for delivery at the start of the next tick. Safe to
    /// call from any thread, any number of times; events arrive in send
    /// order.
    pub fn send<E: 'static + Send + Sync>(&self, event: E) {
        self.tx.emit(Box::new(move |resources| {
            if resources.contains::<Events<E>>() {
                resources.get_mut::<Events<E>>().send(event);
            } else {
                tracing::warn!(
                    "AppHandle::send: {} was never registered via add_event — event dropped",
                    std::any::type_name::<E>()
                );
            }
        }));
    }
}

/// The central application object: owns the ECS world, resources, and every
/// registered system, organized into [`SystemStage`]s.
///
/// Built by chaining `.add_plugin(...)`/`.add_system(...)`/etc. calls —
/// every builder method takes `self` by value and returns `Self`, so a
/// typical setup reads as one expression ending in [`App::run`]:
///
/// ```ignore
/// App::new()
///     .add_plugin(GraphicsPlugin)
///     .add_system(SystemStage::Ready, setup)
///     .add_system(SystemStage::Update, my_game_logic)
///     .run();
/// ```
pub struct App {
    world: hecs::World,
    resources: Resources,
    schedules: BTreeMap<SystemStage, Schedule>,
    pub(crate) gpu_schedules: BTreeMap<SystemStage, Schedule>,
    runner: Option<Box<dyn FnOnce(App)>>,
    handle: AppHandle,
    deliveries: Stream<Delivery>,
}

impl Default for App {
    fn default() -> Self {
        let (tx, deliveries) = Stream::new();

        let mut resources = Resources::default();
        resources.insert(hecs::CommandBuffer::default());
        resources.insert(ResourceCommandQueue::default());
        resources.insert(TriggerQueue::default());
        resources.insert(AppExit::default());
        resources.insert(BackendReady::default());

        Self {
            world: hecs::World::default(),
            schedules: BTreeMap::new(),
            gpu_schedules: BTreeMap::new(),
            resources,
            runner: None,
            handle: AppHandle { tx },
            deliveries,
        }
    }
}

impl App {
    /// Creates a fresh `App` with no plugins, systems, or resources beyond
    /// the small set every app needs internally (a command buffer,
    /// [`AppExit`], [`BackendReady`]). Nothing is registered automatically
    /// — windowing, the GPU backend, `Time` are all opt-in via
    /// `.add_plugin(...)`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a resource, replacing any existing value of the same type.
    pub fn insert_resource<T: 'static>(mut self, resource: T) -> Self {
        self.resources.insert(resource);
        self
    }

    /// Removes a resource, if present. A no-op if it wasn't there.
    pub fn remove_resource<T: 'static>(mut self) -> Self {
        self.resources.remove::<T>();
        self
    }

    /// Runs a [`Plugin`]'s `build`, which may insert resources, register
    /// systems, or add further plugins of its own.
    pub fn add_plugin<P: Plugin>(self, plugin: P) -> Self {
        plugin.build(self)
    }

    fn add_system_to<S, Params>(
        schedules: &mut BTreeMap<SystemStage, Schedule>,
        stage: SystemStage,
        system: impl Into<SystemConfig<S, Params>>,
    ) where
        Params: 'static,
        S: IntoSystem<Params> + 'static,
    {
        schedules
            .entry(stage)
            .or_insert_with(Schedule::default)
            .add_system(system);
    }

    fn add_systems_to(schedules: &mut BTreeMap<SystemStage, Schedule>, stage: SystemStage, chain: SystemChain) {
        schedules
            .entry(stage)
            .or_insert_with(Schedule::default)
            .add_systems(chain);
    }

    /// Registers `system` to run on `stage`, every tick that stage runs.
    /// See [`SystemStage`] for what each stage is for and when it runs.
    /// `system` may be a bare system, or one wrapped with
    /// `.after(...)`/`.before(...)`/`.priority(...)` to order/prioritize it
    /// relative to another system on the same stage — see
    /// [`IntoSystemConfig`](crate::ecs::system_param::IntoSystemConfig).
    pub fn add_system<S, Params>(mut self, stage: SystemStage, system: impl Into<SystemConfig<S, Params>>) -> Self
    where
        Params: 'static,
        S: IntoSystem<Params> + 'static,
    {
        Self::add_system_to(&mut self.schedules, stage, system);
        self
    }

    /// Registers every system in `chain` — built by calling `.chain()` on a
    /// tuple of systems, see [`Chain`](crate::ecs::system_param::Chain) —
    /// to run on `stage`, in that exact relative order.
    pub fn add_systems(mut self, stage: SystemStage, chain: SystemChain) -> Self {
        Self::add_systems_to(&mut self.schedules, stage, chain);
        self
    }

    pub(crate) fn add_gpu_system<S, Params>(mut self, stage: SystemStage, system: impl Into<SystemConfig<S, Params>>) -> Self
    where
        Params: 'static,
        S: IntoSystem<Params> + 'static,
    {
        Self::add_system_to(&mut self.gpu_schedules, stage, system);
        self
    }

    /// Registers event type `T`, making [`EventReader<T>`](crate::ecs::events::EventReader)/
    /// [`EventWriter<T>`](crate::ecs::events::EventWriter) usable as system
    /// parameters. Idempotent — calling this twice for the same `T` (e.g.
    /// from two different plugins that both want it) is a no-op the second
    /// time, not a double registration.
    pub fn add_event<T: 'static + Send + Sync>(mut self) -> Self {
        if !self.resources.contains::<Events<T>>() {
            self.resources.insert(Events::<T>::default());
            self = self.add_system(SystemStage::PreUpdate, age_events::<T>);
        }
        self
    }

    /// Registers `observer` to run whenever [`Commands::trigger`](crate::ecs::commands::Commands::trigger)
    /// sends an `E`, once the current stage finishes syncing (same tick,
    /// not deferred to the next one). Multiple observers can be registered
    /// for the same `E` — every one of them runs.
    pub fn add_observer<E: 'static + Send + Sync, Params: 'static>(
        mut self,
        observer: impl IntoObserverSystem<E, Params> + 'static,
    ) -> Self {
        if !self.resources.contains::<Observers<E>>() {
            self.resources.insert(Observers::<E>::default());
        }
        self.resources.get_mut::<Observers<E>>().0.push(Box::new(observer.into_observer_system()));
        self
    }

    /// Returns a handle for sending events into this app from outside the
    /// ECS — background threads, async tasks, browser callbacks. Grab it
    /// (and clone it freely) before [`App::run`] consumes the app. See
    /// [`AppHandle`].
    pub fn handle(&self) -> AppHandle {
        self.handle.clone()
    }

    /// Overrides how the main loop is driven — e.g. a windowing plugin
    /// installs one that hands control to its own event loop instead of
    /// the default headless polling loop.
    pub fn set_runner(mut self, runner: impl FnOnce(App) + 'static) -> Self {
        self.runner = Some(Box::new(runner));
        self
    }

    /// Initializes a `tracing_subscriber` formatter so `tracing::info!`/
    /// `warn!`/`error!` calls made throughout the engine actually print
    /// somewhere.
    pub fn with_logging(self) -> Self {
        tracing_subscriber::fmt().init();
        self
    }

    /// Runs every schedule for a single tick: while the GPU backend isn't
    /// ready yet, only the internal `gpu_schedules` run; once it is,
    /// [`SystemStage::Ready`] runs (if anything is still registered there,
    /// exactly once ever), then every other stage runs in order. Called
    /// automatically by the default loop in [`App::run`] — call it
    /// yourself only if you're driving the loop from somewhere else (e.g.
    /// inside a custom runner installed via [`App::set_runner`]).
    pub fn update(&mut self) {
        // Deliver externally-queued events (AppHandle::send) before any
        // schedule runs — every reader, whatever its stage, sees the event
        // during this tick and its cursor guarantees exactly-once.
        while let StreamState::Ready(deliver) = self.deliveries.poll() {
            deliver(&self.resources);
        }

        if !self.resources.get::<BackendReady>().0 {
            for (_, schedule) in self.gpu_schedules.iter_mut() {
                schedule.run(&mut self.world, &mut self.resources);
            }
            return;
        }

        if let Some(mut ready) = self.schedules.remove(&SystemStage::Ready) {
            ready.run(&mut self.world, &mut self.resources);
        }
        for (_, schedule) in self.schedules.iter_mut() {
            schedule.run(&mut self.world, &mut self.resources);
        }
    }

    /// `true` once [`AppExit`] has been set — the default loop in
    /// [`App::run`] checks this after every tick.
    pub fn should_exit(&self) -> bool {
        self.resources.get::<AppExit>().0
    }

    /// Whether the GPU backend has finished coming up.
    ///
    /// Until it has, [`App::update`] runs only the internal GPU schedules and
    /// returns, so no system of yours has run yet. A runner that draws on
    /// demand has to keep asking for frames while this is `false`: nothing else
    /// is in a position to ask, and the device is acquired asynchronously, so
    /// the loop would otherwise go to sleep waiting for something whose arrival
    /// it has arranged never to notice.
    pub fn backend_ready(&self) -> bool {
        self.resources.get::<BackendReady>().0
    }

    /// Consumes the app and runs it. [`SystemStage::Startup`] runs first,
    /// exactly once, before anything else. Then, if a runner was installed
    /// (e.g. by a windowing plugin via [`App::set_runner`]), control is
    /// handed to it — this call doesn't return until that runner decides
    /// to stop. Otherwise, falls back to a default headless loop that
    /// calls [`App::update`] repeatedly until [`App::should_exit`].
    pub fn run(mut self) {
        // startup schedules run exactly once, before the main loop
        if let Some(mut startup) = self.schedules.remove(&SystemStage::Startup) {
            startup.run(&mut self.world, &mut self.resources);
        }

        // a windowing plugin (e.g. WindowPlugin) hands control to its own
        // event loop instead of the default headless polling loop below
        if let Some(runner) = self.runner.take() {
            runner(self);
            return;
        }

        loop {
            let was_ready = self.resources.get::<BackendReady>().0;
            self.update();

            if self.should_exit() {
                break;
            }

            if !was_ready {
                // no real OS thread to sleep on wasm32 — just busy-poll.
                // Only reached at all when an app never registers a
                // windowing plugin, which always overrides this runner.
                #[cfg(not(target_arch = "wasm32"))]
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{
        commands::Commands,
        events::{EventReader, EventWriter},
        local::Local,
        observers::Trigger,
        resources::{Read, Write},
    };

    struct Damage(u32);

    #[derive(Default)]
    struct Seen(Vec<u32>);

    fn send_once(mut writer: EventWriter<Damage>, mut sent: Local<bool>) {
        if !*sent {
            writer.send(Damage(7));
            *sent = true;
        }
    }

    fn record(mut reader: EventReader<Damage>, mut seen: Write<Seen>) {
        for event in reader.iter() {
            seen.0.push(event.0);
        }
    }

    #[test]
    fn add_event_called_twice_still_delivers_exactly_once_one_tick_later() {
        // Registering the same event type twice (e.g. two plugins both
        // wanting `Damage`) must be a no-op the second time — this is the
        // regression test for the bug that motivated moving event aging
        // onto the ordinary PreUpdate schedule instead of a special lane:
        // double-registering used to age the buffers twice per tick and
        // silently drop this event before `record` ever saw it.
        let mut app = App::new()
            .add_event::<Damage>()
            .add_event::<Damage>()
            .insert_resource(Seen::default())
            .add_system(SystemStage::PreUpdate, record)
            .add_system(SystemStage::Update, send_once);
        app.resources.get_mut::<BackendReady>().0 = true;

        app.update(); // tick 1: reader runs before the writer sends this tick
        assert_eq!(app.resources.get::<Seen>().0, Vec::<u32>::new());

        app.update(); // tick 2: reader catches last tick's send, exactly once
        assert_eq!(app.resources.get::<Seen>().0, vec![7]);

        app.update(); // tick 3: event has aged out
        assert_eq!(app.resources.get::<Seen>().0, vec![7]);
    }

    struct Ping(u32);

    fn fire_once(mut commands: Commands, mut sent: Local<bool>) {
        if !*sent {
            commands.trigger(Ping(3));
            *sent = true;
        }
    }

    fn on_ping(trigger: Trigger<Ping>, mut seen: Write<Seen>) {
        seen.0.push(trigger.0);
    }

    fn on_ping_doubled(trigger: Trigger<Ping>, mut seen: Write<Seen>) {
        seen.0.push(trigger.0 * 2);
    }

    #[test]
    fn observer_fires_the_same_tick_it_is_triggered() {
        let mut app = App::new()
            .add_observer(on_ping)
            .insert_resource(Seen::default())
            .add_system(SystemStage::Update, fire_once);
        app.resources.get_mut::<BackendReady>().0 = true;

        app.update();
        assert_eq!(app.resources.get::<Seen>().0, vec![3]);

        app.update(); // fire_once no longer sends — nothing new triggered
        assert_eq!(app.resources.get::<Seen>().0, vec![3]);
    }

    #[test]
    fn ready_stage_runs_exactly_once_before_the_regular_schedules_that_same_tick() {
        struct SetupRan;

        fn setup(mut commands: Commands, mut seen: Write<Seen>) {
            seen.0.push(1);
            commands.insert_resource(SetupRan);
        }

        fn depends_on_setup(ran: Option<Read<SetupRan>>, mut seen: Write<Seen>) {
            if ran.is_some() {
                seen.0.push(2);
            }
        }

        let mut app = App::new()
            .add_system(SystemStage::Ready, setup)
            .insert_resource(Seen::default())
            .add_system(SystemStage::PreUpdate, depends_on_setup);

        // not ready yet — Ready must not run before BackendReady
        app.update();
        assert_eq!(app.resources.get::<Seen>().0, Vec::<u32>::new());

        app.resources.get_mut::<BackendReady>().0 = true;

        // same tick: setup runs, commands sync, then depends_on_setup
        // already sees SetupRan — not one tick later
        app.update();
        assert_eq!(app.resources.get::<Seen>().0, vec![1, 2]);

        // never runs again
        app.update();
        assert_eq!(app.resources.get::<Seen>().0, vec![1, 2, 2]);
    }

    #[test]
    fn handle_sends_are_delivered_next_tick_exactly_once() {
        let mut app = App::new()
            .add_event::<Damage>()
            .insert_resource(Seen::default())
            .add_system(SystemStage::PreUpdate, record);
        app.resources.get_mut::<BackendReady>().0 = true;
        let handle = app.handle();

        handle.send(Damage(7));
        handle.send(Damage(9));

        app.update(); // both arrive this tick, in send order
        assert_eq!(app.resources.get::<Seen>().0, vec![7, 9]);

        app.update(); // and never again
        assert_eq!(app.resources.get::<Seen>().0, vec![7, 9]);
    }

    #[test]
    fn handle_send_from_another_thread_is_delivered() {
        let mut app = App::new()
            .add_event::<Damage>()
            .insert_resource(Seen::default())
            .add_system(SystemStage::PreUpdate, record);
        app.resources.get_mut::<BackendReady>().0 = true;
        let handle = app.handle();

        std::thread::spawn(move || handle.send(Damage(3))).join().unwrap();

        app.update();
        assert_eq!(app.resources.get::<Seen>().0, vec![3]);
    }

    #[test]
    fn handle_send_of_an_unregistered_event_type_is_dropped_not_a_panic() {
        struct NeverRegistered;

        let mut app = App::new();
        app.resources.get_mut::<BackendReady>().0 = true;
        let handle = app.handle();

        handle.send(NeverRegistered);

        app.update(); // must not panic on the missing Events<NeverRegistered>
    }

    #[test]
    fn multiple_observers_for_the_same_event_all_fire() {
        let mut app = App::new()
            .add_observer(on_ping)
            .add_observer(on_ping_doubled)
            .insert_resource(Seen::default())
            .add_system(SystemStage::Update, fire_once);
        app.resources.get_mut::<BackendReady>().0 = true;

        app.update();

        let seen = app.resources.get::<Seen>().0.clone();
        assert_eq!(seen.len(), 2);
        assert!(seen.contains(&3));
        assert!(seen.contains(&6));
    }
}
