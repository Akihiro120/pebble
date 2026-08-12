use crate::ecs::{
    resources::{Read, Resources, Write},
    system_param::SystemParam,
};

pub struct Events<T> {
    current: Vec<(usize, T)>,
    previous: Vec<(usize, T)>,
    next_id: usize,
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self { current: Vec::new(), previous: Vec::new(), next_id: 0 }
    }
}

impl<T> Events<T> {
    pub fn send(&mut self, event: T) {
        self.current.push((self.next_id, event));
        self.next_id += 1;
    }

    pub(crate) fn update(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }
}

pub struct EventReader<'a, T: 'static> {
    events: Read<'a, Events<T>>,
    last_seen: &'a mut usize,
}

impl<'a, T: 'static> EventReader<'a, T> {
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

    pub fn is_empty(&self) -> bool {
        let seen = *self.last_seen;
        !self.events.previous.iter().chain(self.events.current.iter()).any(|(id, _)| *id >= seen)
    }
}

impl<T: 'static> SystemParam for EventReader<'static, T> {
    type Item<'a> = EventReader<'a, T>;
    type State = usize;

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources, state: &'a mut Self::State) -> Self::Item<'a> {
        EventReader { events: Read { inner: resources.get::<Events<T>>() }, last_seen: state }
    }
}

impl<T> SystemParam for Option<EventReader<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<EventReader<'a, T>>;
    type State = usize;

    fn fetch<'a>(world: &'a hecs::World, resources: &'a Resources, state: &'a mut Self::State) -> Self::Item<'a> {
        resources.contains::<Events<T>>().then(|| EventReader::fetch(world, resources, state))
    }
}

pub struct EventWriter<'a, T: 'static> {
    events: Write<'a, Events<T>>,
}

impl<'a, T: 'static> EventWriter<'a, T> {
    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }
}

impl<T: 'static> SystemParam for EventWriter<'static, T> {
    type Item<'a> = EventWriter<'a, T>;
    type State = ();

    fn fetch<'a>(_world: &'a hecs::World, resources: &'a Resources, _state: &'a mut Self::State) -> Self::Item<'a> {
        EventWriter { events: Write { inner: resources.get_mut::<Events<T>>() } }
    }
}

impl<T> SystemParam for Option<EventWriter<'static, T>>
where
    T: 'static + Sync + Send,
{
    type Item<'a> = Option<EventWriter<'a, T>>;
    type State = ();

    fn fetch<'a>(world: &'a hecs::World, resources: &'a Resources, state: &'a mut Self::State) -> Self::Item<'a> {
        resources.contains::<Events<T>>().then(|| EventWriter::fetch(world, resources, state))
    }
}

pub(crate) fn age_events<T: Send + Sync + 'static>(mut events: Write<Events<T>>) {
    events.update();
}
