//! A simplified implementation of the classic game "Bomberman".

use bevy::{
    prelude::*,
    sprite::collide_aabb::{collide, Collision},
    time::FixedTimestep,
    utils::HashMap,
};

use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};

// Defines the amount of time that should elapse between each physics step.
const TIME_STEP: f32 = 1.0 / 60.0;

const WALL_THICKNESS: f32 = 10.0;
// x coordinates
const RIGHT_WALL: f32 = BRICK_SIZE.x * (COLS as f32) / 2.;
const LEFT_WALL: f32 = -RIGHT_WALL;
// y coordinates
const TOP_WALL: f32 = BRICK_SIZE.y * (ROWS as f32) / 2.;
const BOTTOM_WALL: f32 = -TOP_WALL;

const BRICK_SIZE: Vec2 = Vec2::new(50., 50.);
const BOMB_SIZE: Vec2 = Vec2::new(40., 40.);
const PLAYER_SIZE: Vec2 = Vec2::new(40., 40.);

const MOVE_SPEED_X: f32 = BRICK_SIZE.x / 10.;
const MOVE_SPEED_Y: f32 = BRICK_SIZE.y / 10.;

const SCOREBOARD_FONT_SIZE: f32 = 40.0;
const SCOREBOARD_TEXT_PADDING: Val = Val::Px(5.0);
const GAMEOVER_FONT_SIZE: f32 = 400.0;

const BACKGROUND_COLOR: Color = Color::rgb(0.9, 0.9, 0.9);
const PLAYER_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);
const OPPONENT_COLOR: Color = Color::rgb(0.4, 0.4, 0.6);
const BRICK_COLOR: Color = Color::rgb(0.4, 0.0, 0.0);
const WALL_COLOR: Color = Color::rgb(0.8, 0.8, 0.8);
const TEXT_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const SCORE_COLOR: Color = Color::rgb(1.0, 0.5, 0.5);
const BOMB_COLOR: Color = Color::rgb(0.0, 0.0, 0.0);
const FIRE_COLOR: Color = Color::rgb(1.0, 0.0, 0.0);

// standard bomberman stage
const ROWS: usize = 11;
const COLS: usize = 13;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Scoreboard::default())
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_startup_system(setup)
        .add_event::<ExplosionEvent>()
        .add_event::<Explosion2Event>()
        .add_event::<MoveEvent>()
        .add_event::<BombEvent>()
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(check_for_explosions)
                .with_system(move_player.before(check_for_explosions))
                .with_system(move_event.after(move_player))
                .with_system(move_opponents.before(move_event))
                .with_system(place_bomb.before(check_for_explosions))
                .with_system(explode.after(check_for_explosions))
                .with_system(explode2.after(explode))
                .with_system(fire.after(explode)),
        )
        .add_system(update_scoreboard)
        .add_system(bevy::window::close_on_esc)
        .run();
}

#[derive(Component)]
struct Player {
    max_bombs: u8,
    active_bombs: u8,
    bomb_power: u8,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            max_bombs: 1,
            active_bombs: 0,
            bomb_power: 1,
        }
    }
}

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Component)]
struct Active;

#[derive(Component)]
struct Breakable;

struct ExplosionEvent(Entity);

struct Explosion2Event(Entity);

struct MoveEvent {
    direction: Collision,
    player: Entity,
}

struct BombEvent {
    player: Entity,
}

#[derive(Component)]
struct Brick;

#[derive(Component)]
struct Bomb {
    player: Entity,
    timer: Timer,
    power: u8,
}

#[derive(Component)]
struct Fire(Timer);

// This resource tracks the game's score
#[derive(Default)]
struct Scoreboard {
    score: usize,
}

// This bundle is a collection of the components that define a "wall" in our game
#[derive(Bundle)]
struct WallBundle {
    // You can nest bundles inside of other bundles like this
    // Allowing you to compose their functionality
    #[bundle]
    sprite_bundle: SpriteBundle,
}

/// Which side of the arena is this wall located on?
enum WallLocation {
    Left,
    Right,
    Bottom,
    Top,
}

impl WallLocation {
    fn position(&self) -> Vec2 {
        match self {
            WallLocation::Left => Vec2::new(LEFT_WALL - WALL_THICKNESS / 2., 0.),
            WallLocation::Right => Vec2::new(RIGHT_WALL + WALL_THICKNESS / 2., 0.),
            WallLocation::Bottom => Vec2::new(0., BOTTOM_WALL - WALL_THICKNESS / 2.),
            WallLocation::Top => Vec2::new(0., TOP_WALL + WALL_THICKNESS / 2.),
        }
    }

    fn size(&self) -> Vec2 {
        let arena_height = TOP_WALL - BOTTOM_WALL;
        let arena_width = RIGHT_WALL - LEFT_WALL;
        // Make sure we haven't messed up our constants
        assert!(arena_height > 0.0);
        assert!(arena_width > 0.0);

        match self {
            WallLocation::Left | WallLocation::Right => {
                Vec2::new(WALL_THICKNESS, arena_height + WALL_THICKNESS)
            }
            WallLocation::Bottom | WallLocation::Top => {
                Vec2::new(arena_width + WALL_THICKNESS, WALL_THICKNESS)
            }
        }
    }
}

impl WallBundle {
    // This "builder method" allows us to reuse logic across our wall entities,
    // making our code easier to read and less prone to bugs when we change the logic
    fn new(location: WallLocation) -> WallBundle {
        WallBundle {
            sprite_bundle: SpriteBundle {
                transform: Transform {
                    // We need to convert our Vec2 into a Vec3, by giving it a z-coordinate
                    // This is used to determine the order of our sprites
                    translation: location.position().extend(0.0),
                    // The z-scale of 2D objects must always be 1.0,
                    // or their ordering will be affected in surprising ways.
                    // See https://github.com/bevyengine/bevy/issues/4149
                    scale: location.size().extend(1.0),
                    ..default()
                },
                sprite: Sprite {
                    color: WALL_COLOR,
                    ..default()
                },
                ..default()
            },
        }
    }
}

// Add the game's entities to our world
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn_bundle(Camera2dBundle::default());

    commands
        .spawn()
        .insert(Player::default())
        .insert_bundle(SpriteBundle {
            transform: Transform {
                // TODO: define starting point
                translation: Vec3::new(
                    LEFT_WALL + BRICK_SIZE.x / 2.,
                    TOP_WALL - BRICK_SIZE.y / 2.,
                    0.0,
                ),
                scale: PLAYER_SIZE.extend(0.0),
                ..default()
            },
            sprite: Sprite {
                color: PLAYER_COLOR,
                ..default()
            },
            ..default()
        })
        .insert(Active);

    commands
        .spawn()
        .insert(Player::default())
        .insert_bundle(SpriteBundle {
            transform: Transform {
                // TODO: define starting point
                translation: Vec3::new(
                    RIGHT_WALL - BRICK_SIZE.x / 2.,
                    TOP_WALL - BRICK_SIZE.y / 2.,
                    0.0,
                ),
                scale: PLAYER_SIZE.extend(0.0),
                ..default()
            },
            sprite: Sprite {
                color: OPPONENT_COLOR,
                ..default()
            },
            ..default()
        });

    commands
        .spawn()
        .insert(Player::default())
        .insert_bundle(SpriteBundle {
            transform: Transform {
                // TODO: define starting point
                translation: Vec3::new(
                    RIGHT_WALL - BRICK_SIZE.x / 2.,
                    BOTTOM_WALL + BRICK_SIZE.y / 2.,
                    0.0,
                ),
                scale: PLAYER_SIZE.extend(0.0),
                ..default()
            },
            sprite: Sprite {
                color: OPPONENT_COLOR,
                ..default()
            },
            ..default()
        });

    commands
        .spawn()
        .insert(Player::default())
        .insert_bundle(SpriteBundle {
            transform: Transform {
                // TODO: define starting point
                translation: Vec3::new(
                    LEFT_WALL + BRICK_SIZE.x / 2.,
                    BOTTOM_WALL + BRICK_SIZE.y / 2.,
                    0.0,
                ),
                scale: PLAYER_SIZE.extend(0.0),
                ..default()
            },
            sprite: Sprite {
                color: OPPONENT_COLOR,
                ..default()
            },
            ..default()
        });

    // Scoreboard
    commands.spawn_bundle(
        TextBundle::from_sections([
            TextSection::new(
                "Score: ",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: SCOREBOARD_FONT_SIZE,
                    color: TEXT_COLOR,
                },
            ),
            TextSection::from_style(TextStyle {
                font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                font_size: SCOREBOARD_FONT_SIZE,
                color: SCORE_COLOR,
            }),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            position: UiRect {
                top: SCOREBOARD_TEXT_PADDING,
                left: SCOREBOARD_TEXT_PADDING,
                ..default()
            },
            ..default()
        }),
    );

    // Walls
    commands.spawn_bundle(WallBundle::new(WallLocation::Left));
    commands.spawn_bundle(WallBundle::new(WallLocation::Right));
    commands.spawn_bundle(WallBundle::new(WallLocation::Bottom));
    commands.spawn_bundle(WallBundle::new(WallLocation::Top));

    // In Bevy, the `translation` of an entity describes the center point,
    // not its bottom-left corner
    let offset_x = LEFT_WALL + BRICK_SIZE.x / 2.;
    let offset_y = BOTTOM_WALL + BRICK_SIZE.y / 2.;
    for row in 0..ROWS {
        for col in 0..COLS {
            let brick_position = Vec2::new(
                offset_x + (col as f32) * BRICK_SIZE.x,
                offset_y + (row as f32) * BRICK_SIZE.y,
            );

            // TODO: manage different dispositions
            if row % 2 == 1 && col % 2 == 1 {
                // brick
                commands.spawn().insert(Brick).insert_bundle(SpriteBundle {
                    sprite: Sprite {
                        color: WALL_COLOR,
                        ..default()
                    },
                    transform: Transform {
                        translation: brick_position.extend(0.0),
                        scale: Vec3::new(BRICK_SIZE.x, BRICK_SIZE.y, 1.0),
                        ..default()
                    },
                    ..default()
                });
            }
            // TODO: randomly dispose walls
            else if (2..(ROWS - 2)).contains(&row) || (2..(COLS - 2)).contains(&col) {
                // wall
                commands
                    .spawn()
                    .insert(Brick)
                    .insert_bundle(SpriteBundle {
                        sprite: Sprite {
                            color: BRICK_COLOR,
                            ..default()
                        },
                        transform: Transform {
                            translation: brick_position.extend(0.0),
                            scale: Vec3::new(BRICK_SIZE.x, BRICK_SIZE.y, 1.0),
                            ..default()
                        },
                        ..default()
                    })
                    .insert(Breakable);
            }
        }
    }
}

fn move_player(
    keyboard_input: Res<Input<KeyCode>>,
    mut move_writer: EventWriter<MoveEvent>,
    mut bomb_writer: EventWriter<BombEvent>,
    query: Query<Entity, (With<Player>, With<Active>)>,
) {
    if let Ok(player) = query.get_single() {
        for key in keyboard_input.get_pressed() {
            match key {
                KeyCode::Up => {
                    move_writer.send(MoveEvent {
                        direction: Collision::Top,
                        player,
                    });
                }
                KeyCode::Down => {
                    move_writer.send(MoveEvent {
                        direction: Collision::Bottom,
                        player,
                    });
                }
                KeyCode::Right => {
                    move_writer.send(MoveEvent {
                        direction: Collision::Right,
                        player,
                    });
                }
                KeyCode::Left => {
                    move_writer.send(MoveEvent {
                        direction: Collision::Left,
                        player,
                    });
                }
                KeyCode::Space => {
                    bomb_writer.send(BombEvent { player });
                }
                _ => {}
            }
        }
    }
}

fn move_opponents(
    mut move_writer: EventWriter<MoveEvent>,
    mut bomb_writer: EventWriter<BombEvent>,
    query: Query<Entity, (With<Player>, Without<Active>)>,
) {
    let mut rng = thread_rng();
    let between = Uniform::from(0_u8..5_u8);
    for player in &query {
        match between.sample(&mut rng) {
            0 => {
                move_writer.send(MoveEvent {
                    direction: Collision::Bottom,
                    player,
                });
            }
            1 => {
                move_writer.send(MoveEvent {
                    direction: Collision::Left,
                    player,
                });
            }
            2 => {
                move_writer.send(MoveEvent {
                    direction: Collision::Right,
                    player,
                });
            }
            3 => {
                move_writer.send(MoveEvent {
                    direction: Collision::Top,
                    player,
                });
            }
            _ => {
                bomb_writer.send(BombEvent { player });
            }
        }
    }
}

fn move_event(
    mut event_reader: EventReader<MoveEvent>,
    collision_query: Query<&Transform, (With<Brick>, Without<Player>)>,
    mut query: Query<(Entity, &mut Transform), With<Player>>,
) {
    let mut players = HashMap::new();
    for (entity, transform) in &mut query {
        players.insert(entity, transform);
    }

    for MoveEvent { direction, player } in event_reader.iter() {
        let player_transform = if let Some(t) = players.get_mut(player) {
            t
        } else {
            continue;
        };

        let mut new_translation = player_transform.translation.clone();
        match direction {
            Collision::Top => {
                new_translation.y =
                    (TOP_WALL - BRICK_SIZE.y / 2.).min(new_translation.y + MOVE_SPEED_Y);
            }
            Collision::Bottom => {
                new_translation.y =
                    (BOTTOM_WALL + BRICK_SIZE.y / 2.).max(new_translation.y - MOVE_SPEED_Y);
            }
            Collision::Right => {
                new_translation.x =
                    (RIGHT_WALL - BRICK_SIZE.x / 2.).min(new_translation.x + MOVE_SPEED_X);
            }
            Collision::Left => {
                new_translation.x =
                    (LEFT_WALL + BRICK_SIZE.x / 2.).max(new_translation.x - MOVE_SPEED_X);
            }
            _ => {}
        }

        let player_size = player_transform.scale.truncate();
        let (mut collide_up, mut collide_down, mut collide_right, mut collide_left) =
            (false, false, false, false);
        for brick_transform in &collision_query {
            if let Some(collision) = collide(
                new_translation,
                player_size,
                brick_transform.translation,
                brick_transform.scale.truncate(),
            ) {
                match collision {
                    Collision::Top => collide_down = true,
                    Collision::Bottom => collide_up = true,
                    Collision::Left => collide_right = true,
                    Collision::Right => collide_left = true,
                    _ => {}
                }
            }
        }

        if !collide_up && !collide_down {
            player_transform.translation.y = new_translation.y;
        }
        if !collide_left && !collide_right {
            player_transform.translation.x = new_translation.x;
        }
    }
}

fn place_bomb(
    mut commands: Commands,
    mut event_reader: EventReader<BombEvent>,
    mut query: Query<(Entity, &mut Player, &Transform), With<Player>>,
) {
    let mut players = HashMap::new();
    for (entity, player, transform) in &mut query {
        if player.active_bombs >= player.max_bombs {
            continue;
        }
        players.insert(entity, (player, transform));
    }

    for BombEvent {
        player: player_entity,
    } in event_reader.iter()
    {
        let (player, player_transform) = if let Some(t) = players.get_mut(player_entity) {
            t
        } else {
            continue;
        };

        let mut bomb_translation = player_transform.translation.clone();
        bomb_translation.x = BRICK_SIZE.x * (bomb_translation.x / BRICK_SIZE.x).round();
        bomb_translation.y = BRICK_SIZE.y * (bomb_translation.y / BRICK_SIZE.y).round();

        commands
            .spawn()
            .insert(Bomb {
                player: *player_entity,
                timer: Timer::from_seconds(1., false),
                power: player.bomb_power,
            })
            .insert_bundle(SpriteBundle {
                sprite: Sprite {
                    color: BOMB_COLOR,
                    ..default()
                },
                transform: Transform {
                    translation: bomb_translation,
                    scale: Vec3::new(BOMB_SIZE.x, BOMB_SIZE.y, 1.0),
                    ..default()
                },
                ..default()
            });

        player.active_bombs += 1;
    }
}

fn check_for_explosions(
    mut query: Query<(Entity, &mut Bomb), (Without<Brick>, Without<Player>, With<Bomb>)>,
    time: Res<Time>,
    mut explosion_events: EventWriter<ExplosionEvent>,
) {
    for (bomb_entity, mut bomb) in &mut query {
        bomb.timer.tick(time.delta());
        if bomb.timer.finished() {
            explosion_events.send(ExplosionEvent(bomb_entity));
        }
    }
}

fn explode(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut scoreboard: ResMut<Scoreboard>,
    bomb_collision_query: Query<
        (Entity, &Bomb, &Transform),
        (Without<Brick>, Without<Player>, With<Bomb>),
    >,
    brick_collision_query: Query<
        (Entity, &Transform),
        (With<Brick>, With<Breakable>, Without<Player>, Without<Bomb>),
    >,
    mut player_collision_query: Query<
        (Entity, &Transform, &mut Player, Option<With<Active>>),
        (Without<Brick>, With<Player>, Without<Bomb>),
    >,
    mut event_reader: EventReader<ExplosionEvent>,
    mut event_writer: EventWriter<Explosion2Event>,
) {
    for event in event_reader.iter() {
        let bomb_entity = event.0;

        if let Some((_, bomb, bomb_transform)) = bomb_collision_query
            .iter()
            .find(|(other_bomb_entity, _, _)| other_bomb_entity == &bomb_entity)
        {
            // bomb
            for (other_bomb_entity, _other_bomb, other_bomb_transform) in &bomb_collision_query {
                if bomb_entity == other_bomb_entity {
                    continue;
                }

                // horizontal
                if collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x * (2. * (bomb.power as f32) + 1.), BRICK_SIZE.y), other_bomb_transform.translation, other_bomb_transform.scale.truncate()).is_some()
                // vertical
                || collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x, BRICK_SIZE.y * (2. * (bomb.power as f32) + 1.)), other_bomb_transform.translation, other_bomb_transform.scale.truncate()).is_some()
                {
                    event_writer.send(Explosion2Event(other_bomb_entity));
                }
            }

            // brick
            for (brick_entity, brick_transform) in &brick_collision_query {
                // horizontal
                if collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x * (2. * (bomb.power as f32) + 1.), BRICK_SIZE.y), brick_transform.translation, brick_transform.scale.truncate()).is_some()
                // vertical
                || collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x, BRICK_SIZE.y * (2. * (bomb.power as f32) + 1.)), brick_transform.translation, brick_transform.scale.truncate()).is_some()
                {
                    scoreboard.score += 1;
                    commands.entity(brick_entity).despawn();
                }
            }

            // player
            for (player_entity, player_transform, mut player, active) in &mut player_collision_query {
                if player_entity == bomb.player {
                    player.active_bombs -= 1;
                }

                // horizontal
                if collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x * (2. * (bomb.power as f32) + 1.), BRICK_SIZE.y), player_transform.translation, player_transform.scale.truncate()).is_some()
                // vertical
                || collide(bomb_transform.translation, Vec2::new(BRICK_SIZE.x, BRICK_SIZE.y * (2. * (bomb.power as f32) + 1.)), player_transform.translation, player_transform.scale.truncate()).is_some()
                {
                    if active.is_some() {
                        game_over(&mut commands, &asset_server);
                    } else {
                        scoreboard.score += 100;
                    }
                    commands.entity(player_entity).despawn();
                }
            }

            // horizontal fire
            commands
                .spawn()
                .insert(Fire(Timer::from_seconds(1., false)))
                .insert_bundle(SpriteBundle {
                    sprite: Sprite {
                        color: FIRE_COLOR,
                        ..default()
                    },
                    transform: Transform {
                        translation: bomb_transform.translation,
                        scale: Vec3::new(
                            BRICK_SIZE.x * (2. * (bomb.power as f32) + 1.),
                            BRICK_SIZE.y,
                            1.0,
                        ),
                        ..default()
                    },
                    ..default()
                });
            // vertical fire
            commands
                .spawn()
                .insert(Fire(Timer::from_seconds(1., false)))
                .insert_bundle(SpriteBundle {
                    sprite: Sprite {
                        color: FIRE_COLOR,
                        ..default()
                    },
                    transform: Transform {
                        translation: bomb_transform.translation,
                        scale: Vec3::new(
                            BRICK_SIZE.x,
                            BRICK_SIZE.y * (2. * (bomb.power as f32) + 1.),
                            1.0,
                        ),
                        ..default()
                    },
                    ..default()
                });
        }

        commands.entity(bomb_entity).despawn();
    }
}

fn fire(
    mut commands: Commands,
    mut fire_query: Query<(Entity, &mut Fire), With<Fire>>,
    time: Res<Time>,
) {
    for (fire_entity, mut fire) in &mut fire_query {
        fire.0.tick(time.delta());
        if fire.0.finished() {
            commands.entity(fire_entity).despawn();
        }
    }
}

fn explode2(
    mut event_reader: EventReader<Explosion2Event>,
    mut event_writer: EventWriter<ExplosionEvent>,
) {
    for event in event_reader.iter() {
        event_writer.send(ExplosionEvent(event.0));
    }
}

fn update_scoreboard(scoreboard: Res<Scoreboard>, mut query: Query<&mut Text>) {
    if let Ok(mut text) = query.get_single_mut() {
        text.sections[1].value = scoreboard.score.to_string();
    }
}

fn game_over(commands: &mut Commands, asset_server: &AssetServer) {
    commands
        .spawn()
        .insert_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
                ..Default::default()
            },
            // material: materials.add(Color::NONE.into()),
            ..Default::default()
        })
        .insert_bundle(TextBundle {
            text: Text {
                sections: vec![TextSection {
                    value: "GAME\nOVER".to_string(),
                    style: TextStyle {
                        font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                        font_size: GAMEOVER_FONT_SIZE,
                        color: TEXT_COLOR,
                    },
                }],
                alignment: TextAlignment {
                    vertical: VerticalAlign::Center,
                    horizontal: HorizontalAlign::Center,
                },
            },
            style: Style {
                align_self: AlignSelf::Center,
                align_content: AlignContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..Default::default()
        });
}
