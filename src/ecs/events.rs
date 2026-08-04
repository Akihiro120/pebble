use crate::ecs::resources::Resources;
use crate::ecs::system::{RequiredResource, Res, SystemParam};
use crate::threading::{BackgroundTasks, SpawnableFuture};

/// A double-buffered queue of `T` events, stored as a singleton resource.
///
/// Write with an [`EventWriter<T>`] system param, read with an
/// [`EventReader<T>`] system param. An event sent during tick `N` stays
/// visible to readers for the rest of tick `N` and all of tick `N + 1`, then
/// is dropped — so a reader running anywhere in either tick sees it exactly
/// once, regardless of whether it runs before or after the writer that sent
/// it.
///
/// Not usable until registered with
/// [`App::add_event`](crate::app::App::add_event), which inserts this
/// resource and schedules the per-tick aging that gives the two-tick
/// guarantee above.
pub struct Events<T> {
    current: Vec<(usize, T)>,
    previous: Vec<(usize, T)>,
    next_id: usize,
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
            next_id: 0,
        }
    }
}

impl<T> Events<T> {
    /// Push a new event, tagged with the next monotonically increasing id.
    pub fn send(&mut self, event: T) {
        self.current.push((self.next_id, event));
        self.next_id += 1;
    }

    /// Age the buffers: last tick's `current` becomes `previous` (still
    /// visible to readers that haven't caught up), and a fresh `current`
    /// starts collecting this tick's sends.
    ///
    /// Called once per tick, before any user systems run, by the closure
    /// [`App::add_event`](crate::app::App::add_event) registers — not meant
    /// to be called directly.
    pub(crate) fn update(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }
}

/// Read-only handle to an [`Events<T>`] resource, obtained as a system
/// parameter.
///
/// Each system with an `EventReader<T>` parameter has its own private
/// read cursor (persisted like [`Local`](crate::ecs::system::Local)), so
/// multiple readers of the same event type don't interfere with each other.
pub struct EventReader<'a, T: 'static> {
    events: hecs::Ref<'a, Events<T>>,
    last_seen: &'a mut usize,
}

impl<'a, T: 'static> EventReader<'a, T> {
    /// Iterate events this reader hasn't seen yet, oldest first, then mark
    /// them as read.
    pub fn iter(&mut self) -> impl Iterator<Item = &T> + '_ {
        let seen = *self.last_seen;
        let unread: Vec<&T> = self
            .events
            .previous
            .iter()
            .chain(self.events.current.iter())
            .filter(|(id, _)| *id >= seen)
            .map(|(_, event)| event)
            .collect();
        *self.last_seen = self.events.next_id;
        unread.into_iter()
    }

    /// `true` if there are no unread events for this reader.
    pub fn is_empty(&self) -> bool {
        let seen = *self.last_seen;
        !self
            .events
            .previous
            .iter()
            .chain(self.events.current.iter())
            .any(|(id, _)| *id >= seen)
    }
}

impl<T> SystemParam for EventReader<'static, T>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = EventReader<'a, T>;
    type State = usize;

    fn fetch<'a>(
        state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        EventReader {
            events: resources.get_resource(world),
            last_seen: state,
        }
    }

    fn requires() -> Vec<RequiredResource> {
        vec![RequiredResource {
            name: std::any::type_name::<Events<T>>(),
            type_id: std::any::TypeId::of::<Events<T>>(),
            present: |world, resources| resources.has_resource::<Events<T>>(world),
            hint: Some(
                "Register this event type with `app.add_event::<T>()` (or \
                 `app.add_async_event::<T>()` if it's delivered by a background task) \
                 before this system runs.",
            ),
        }]
    }
}

/// Soft-access variant of [`EventReader<T>`]: `None` instead of panicking
/// while `T` isn't registered (via [`App::add_event`](crate::app::App::add_event)),
/// so a system that should just skip when an event type it optionally
/// consumes hasn't been set up yet doesn't have to hard-depend on it. The
/// read cursor still persists correctly across ticks once the event type
/// does appear later — it lives in the same per-system `State` slot either way.
impl<T> SystemParam for Option<EventReader<'static, T>>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = Option<EventReader<'a, T>>;
    type State = usize;

    fn fetch<'a>(
        state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        if resources.has_resource::<Events<T>>(world) {
            return Some(EventReader {
                events: resources.get_resource(world),
                last_seen: state,
            });
        }
        None
    }
}

/// Write handle to an [`Events<T>`] resource, obtained as a system
/// parameter.
pub struct EventWriter<'a, T: 'static> {
    events: hecs::RefMut<'a, Events<T>>,
}

impl<'a, T: 'static> EventWriter<'a, T> {
    /// Queue `event` for delivery to every [`EventReader<T>`] for the rest
    /// of this tick and all of the next.
    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }
}

impl<T> SystemParam for EventWriter<'static, T>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = EventWriter<'a, T>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        EventWriter {
            events: resources.get_resource_mut(world),
        }
    }

    fn requires() -> Vec<RequiredResource> {
        vec![RequiredResource {
            name: std::any::type_name::<Events<T>>(),
            type_id: std::any::TypeId::of::<Events<T>>(),
            present: |world, resources| resources.has_resource::<Events<T>>(world),
            hint: Some(
                "Register this event type with `app.add_event::<T>()` (or \
                 `app.add_async_event::<T>()` if it's delivered by a background task) \
                 before this system runs.",
            ),
        }]
    }
}

/// Soft-access variant of [`EventWriter<T>`]: `None` instead of panicking
/// while `T` isn't registered (via [`App::add_event`](crate::app::App::add_event)).
impl<T> SystemParam for Option<EventWriter<'static, T>>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = Option<EventWriter<'a, T>>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        if resources.has_resource::<Events<T>>(world) {
            return Some(EventWriter {
                events: resources.get_resource_mut(world),
            });
        }
        None
    }
}

/// The channel [`AsyncEventWriter::spawn`] sends into and the drain system
/// (registered by [`App::add_async_event`](crate::app::App::add_async_event))
/// reads out of, turning finished background tasks into ordinary `T`
/// events. A resource, but not one to reach for directly — go through
/// [`AsyncEventWriter<T>`].
pub(crate) struct AsyncEventChannel<T> {
    tx: crossbeam_channel::Sender<T>,
    rx: crossbeam_channel::Receiver<T>,
}

impl<T> AsyncEventChannel<T> {
    pub(crate) fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

/// `PreUpdate` system registered by [`App::add_async_event`](crate::app::App::add_async_event):
/// drains every task that finished since last checked and re-delivers each
/// result as a `T` event. Two ticks of latency at most between a task
/// finishing and a reader seeing it — one to be drained here, one more it
/// stays visible for like any [`Events<T>`] send.
pub(crate) fn drain_async_events<T: Send + Sync + 'static>(
    channel: Res<AsyncEventChannel<T>>,
    mut writer: EventWriter<T>,
) {
    while let Ok(event) = channel.rx.try_recv() {
        writer.send(event);
    }
}

/// System parameter for spawning async work whose result is delivered as a
/// `T` event once it resolves. The friendly front door for combining
/// [`BackgroundTasks::spawn_async`] with the [`Events`] system, instead of
/// hand-rolling a pending-task resource and poll system per async data
/// source the way [`GraphicsPlugin`](crate::rendering::graphics_plugin::GraphicsPlugin)
/// does for the GPU backend.
///
/// Register the event type with [`App::add_async_event::<T>()`](crate::app::App::add_async_event)
/// (instead of [`App::add_event`](crate::app::App::add_event)) to make this usable as a system
/// parameter. Reading the result is completely ordinary — just an
/// [`EventReader<T>`], same as any other event; nothing about consuming it
/// differs from a synchronously-sent one. Requires
/// [`BackgroundTasksPlugin`](crate::threading::BackgroundTasksPlugin) to be
/// registered, since [`spawn`](Self::spawn) drives its future through
/// [`BackgroundTasks::spawn_async`].
///
/// ```ignore
/// app.add_async_event::<ReadbackDone>();
///
/// fn start_readback(events: AsyncEventWriter<ReadbackDone>, buffer: Res<SomeGpuBuffer>) {
///     let future = buffer.0.read(); // buffer.0: pebble::wgpu::buffer::Buffer
///     events.spawn(async move { ReadbackDone(future.await) });
/// }
///
/// fn on_readback(mut reader: EventReader<ReadbackDone>) {
///     for event in reader.iter() {
///         // event.0
///     }
/// }
/// ```
pub struct AsyncEventWriter<'a, T: Send + 'static> {
    tasks: hecs::Ref<'a, BackgroundTasks>,
    channel: hecs::Ref<'a, AsyncEventChannel<T>>,
}

impl<'a, T: Send + 'static> AsyncEventWriter<'a, T> {
    /// Spawn `future`. Once it resolves, its output is delivered to every
    /// [`EventReader<T>`] — no matter how many ticks the future takes, and
    /// with no need to hold onto a handle or poll anything yourself.
    pub fn spawn(&self, future: impl SpawnableFuture<T>) {
        let tx = self.channel.tx.clone();
        let _ = self.tasks.spawn_async(async move {
            // Ignore send failure: it only happens if the receiving end
            // (owned by the App) was dropped, meaning the app has shut down
            // and there's nothing left to deliver this to.
            let _ = tx.send(future.await);
        });
    }
}

impl<T> SystemParam for AsyncEventWriter<'static, T>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = AsyncEventWriter<'a, T>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        AsyncEventWriter {
            tasks: resources.get_resource(world),
            channel: resources.get_resource(world),
        }
    }

    fn requires() -> Vec<RequiredResource> {
        vec![
            RequiredResource {
                name: std::any::type_name::<BackgroundTasks>(),
                type_id: std::any::TypeId::of::<BackgroundTasks>(),
                present: |world, resources| resources.has_resource::<BackgroundTasks>(world),
                hint: Some(
                    "`AsyncEventWriter` drives its future through `BackgroundTasks` — register \
                     `app.add_plugin(BackgroundTasksPlugin::new(worker_count))` before this system runs.",
                ),
            },
            RequiredResource {
                name: std::any::type_name::<AsyncEventChannel<T>>(),
                type_id: std::any::TypeId::of::<AsyncEventChannel<T>>(),
                present: |world, resources| resources.has_resource::<AsyncEventChannel<T>>(world),
                hint: Some(
                    "Register this event type with `app.add_async_event::<T>()` (not \
                     `app.add_event`) before this system runs.",
                ),
            },
        ]
    }
}

/// Soft-access variant of [`AsyncEventWriter<T>`]: `None` instead of panicking
/// while `T` isn't registered via [`add_async_event`](crate::app::App::add_async_event)
/// (or [`BackgroundTasksPlugin`](crate::threading::BackgroundTasksPlugin)
/// isn't installed yet). Useful for systems that only opportunistically
/// spawn background work and shouldn't hard-fail while the app is still
/// assembling its plugins.
impl<T> SystemParam for Option<AsyncEventWriter<'static, T>>
where
    T: Send + Sync + 'static,
{
    type Item<'a> = Option<AsyncEventWriter<'a, T>>;
    type State = ();

    fn fetch<'a>(
        _state: &'a mut Self::State,
        world: &'a hecs::World,
        resources: &'a Resources,
    ) -> Self::Item<'a> {
        if resources.has_resource::<BackgroundTasks>(world)
            && resources.has_resource::<AsyncEventChannel<T>>(world)
        {
            return Some(AsyncEventWriter {
                tasks: resources.get_resource(world),
                channel: resources.get_resource(world),
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, SystemStage};
    use crate::ecs::system::{Local, ResMut};

    struct Damage(u32);

    struct Seen(Vec<u32>);

    fn send_once(mut writer: EventWriter<Damage>, mut sent: Local<bool>) {
        if !*sent {
            writer.send(Damage(5));
            *sent = true;
        }
    }

    fn record_damage(mut reader: EventReader<Damage>, mut seen: ResMut<Seen>) {
        for event in reader.iter() {
            seen.0.push(event.0);
        }
    }

    #[test]
    fn reader_in_an_earlier_stage_still_catches_a_same_tick_send_one_tick_later() {
        let mut app = App::new();
        app.add_event::<Damage>();
        app.add_resource(Seen(Vec::new()));

        // The reader runs in PreUpdate, before the writer's Update stage —
        // so within the tick the event is sent, this reader has already run
        // and can't see it yet. It should only pick it up on the *next*
        // tick, via the aged `previous` buffer, and exactly once.
        app.add_system(SystemStage::PreUpdate, record_damage);
        app.add_system(SystemStage::Update, send_once);
        app.build();

        app.update(); // tick 1: reader runs first (nothing sent yet), then writer sends
        assert_eq!(app.get_resource::<Seen>().0, Vec::<u32>::new());

        app.update(); // tick 2: reader now sees last tick's send
        assert_eq!(app.get_resource::<Seen>().0, vec![5]);

        app.update(); // tick 3: event has aged out, nothing new
        assert_eq!(app.get_resource::<Seen>().0, vec![5]);
    }

    struct Loaded(u32);

    fn kick_off(events: AsyncEventWriter<Loaded>, mut sent: Local<bool>) {
        if !*sent {
            events.spawn(async { Loaded(42) });
            *sent = true;
        }
    }

    fn record_loaded(mut reader: EventReader<Loaded>, mut seen: ResMut<Seen>) {
        for event in reader.iter() {
            seen.0.push(event.0);
        }
    }

    #[test]
    fn async_events_deliver_a_background_tasks_result_as_an_event() {
        use crate::ecs::plugin::Plugin;
        use crate::threading::BackgroundTasksPlugin;

        let mut app = App::new();
        BackgroundTasksPlugin::new(1).build(&mut app);
        app.add_async_event::<Loaded>();
        app.add_resource(Seen(Vec::new()));
        app.add_system(SystemStage::Update, kick_off);
        app.add_system(SystemStage::PostRender, record_loaded);
        app.build();

        // The task resolves on a worker thread, off the tick loop — poll a
        // bounded number of ticks (with a short sleep so the worker gets a
        // chance to run) instead of assuming it lands on a specific tick.
        for _ in 0..200 {
            app.update();
            if !app.get_resource::<Seen>().0.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(app.get_resource::<Seen>().0, vec![42]);
    }
}
