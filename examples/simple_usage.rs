use pebble::prelude::*;

#[derive(Default)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Default)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[derive(Default)]
struct GameConfig {
    pub gravity: f32,
}

struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(GameConfig { gravity: 9.8 })
            .add_system(SystemStage::Update, move_entities)
            .add_system(SystemStage::Startup, spawn_player);
    }
}

fn spawn_player(mut commands: Commands) {
    println!("Spawning player...");
    commands.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 1.0 }));
}

fn move_entities(config: Res<GameConfig>, mut query: Query<(&mut Position, &Velocity)>) {
    for (pos, vel) in query.iter() {
        pos.x += vel.dx;
        pos.y += vel.dy + config.gravity;
        println!("Player moved to: {:.2}, {:.2}", pos.x, pos.y);
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugin(GamePlugin {}).build().run(run);
}

fn run(app: &mut App) {
    for _ in 0..5 {
        app.update();
    }
}
