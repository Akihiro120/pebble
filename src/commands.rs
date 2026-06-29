pub enum Command {
    Spawn(hecs::EntityBuilder),
    Despawn(hecs::Entity),
    AddComponent {
        entity: hecs::Entity,
        add_fn: Box<dyn FnOnce(&mut hecs::World, hecs::Entity)>,
    },
    RemoveComponent {
        entity: hecs::Entity,
        remove_fn: Box<dyn FnOnce(&mut hecs::World, hecs::Entity)>,
    },
}

pub struct Commands {
    queue: Vec<Command>,
}

impl Commands {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn spawn(&mut self, bundle: impl hecs::DynamicBundle) {
        let mut builder = hecs::EntityBuilder::new();
        builder.add_bundle(bundle);
        self.queue.push(Command::Spawn(builder));
    }

    pub fn despawn(&mut self, entity: hecs::Entity) {
        self.queue.push(Command::Despawn(entity));
    }

    pub fn insert<T: hecs::Component>(&mut self, entity: hecs::Entity, component: T) {
        self.queue.push(Command::AddComponent {
            entity,
            add_fn: Box::new(move |world, e| {
                world.insert_one(e, component).ok();
            }),
        });
    }

    pub fn remove<T: hecs::Component>(&mut self, entity: hecs::Entity) {
        self.queue.push(Command::RemoveComponent {
            entity,
            remove_fn: Box::new(move |world, e| {
                world.remove_one::<T>(e).ok();
            }),
        });
    }

    pub fn flush(&mut self, world: &mut hecs::World) {
        for command in self.queue.drain(..) {
            match command {
                Command::Spawn(mut builder) => {
                    world.spawn(builder.build());
                }
                Command::Despawn(e) => {
                    world.despawn(e).ok();
                }
                Command::AddComponent { entity, add_fn } => {
                    add_fn(world, entity);
                }
                Command::RemoveComponent { entity, remove_fn } => {
                    remove_fn(world, entity);
                }
            }
        }
    }
}
