use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

use acerbus_common::*;
use bevy::app::AppExit;
use bevy::ecs::schedule::ShouldRun;
use bevy::prelude::shape::Quad;
use bevy::prelude::*;
use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};
use bevy_asset_loader::{AssetCollection, AssetCollectionApp};
use bevy_renet::renet::{ClientAuthentication, RenetClient, RenetConnectionConfig};
use bevy_renet::{run_if_client_conected, RenetClientPlugin};
use clap::Parser;

#[derive(Parser)]
struct Opt {
    #[clap(long, default_value = "127.0.0.1:5000")]
    server_addr: SocketAddr,
}

fn main() {
    let opt = Opt::parse();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_collection::<GameAssets>();
    app.insert_resource(Lobby::default());

    app.add_plugin(RenetClientPlugin);
    app.insert_resource(new_renet_client(opt.server_addr));
    app.insert_resource(PlayerInput::default());
    app.add_system(player_input);
    app.add_system(
        camera_follow_player
            .with_run_criteria(run_if_client_conected)
            .with_run_criteria(run_if_player_exist),
    );
    app.add_system(client_send_input.with_run_criteria(run_if_client_conected));
    app.add_system(client_sync_players.with_run_criteria(run_if_client_conected));

    app.insert_resource(LogRttConfig { timer: Timer::new(Duration::from_secs(5), true) });
    app.add_system(log_rtt.with_run_criteria(run_if_client_conected));

    app.add_startup_system(setup);
    app.add_system_to_stage(CoreStage::PostUpdate, close_connection_exit_system);
    app.add_system(panic_on_error_system);

    app.run();
}

#[derive(AssetCollection)]
struct GameAssets {
    #[asset(path = "images/icon-green.png")]
    icon_green: Handle<Image>,
    #[asset(path = "images/icon-purple.png")]
    icon_purple: Handle<Image>,
}

fn new_renet_client(server_addr: SocketAddr) -> RenetClient {
    let mut socket = server_addr.clone();
    socket.set_port(0);
    let socket = UdpSocket::bind(socket).unwrap();
    let connection_config = RenetConnectionConfig::default();
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let client_id = current_time.as_millis() as u64;
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    RenetClient::new(current_time, socket, client_id, connection_config, authentication).unwrap()
}

fn client_sync_players(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut client: ResMut<RenetClient>,
    mut lobby: ResMut<Lobby>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    while let Some(message) = client.receive_message(CONNECTION_EVENTS_CHANNEL) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessage::PlayerConnected { player } => {
                println!("{:?} connected.", player);

                let player_entity = commands
                    .spawn_bundle(MaterialMesh2dBundle {
                        mesh: Mesh2dHandle(meshes.add(
                            Quad::new(Vec2::new(PLAYER_SQUARE_WIDTH, PLAYER_SQUARE_HEIGHT)).into(),
                        )),
                        material: materials.add(ColorMaterial::from(Color::PURPLE)),
                        ..default()
                    })
                    .insert(player)
                    .id();

                lobby.players.insert(player, player_entity);
            }
            ServerMessage::PlayerDisconnected { player } => {
                println!("{:?} disconnected.", player);
                if let Some(player_entity) = lobby.players.remove(&player) {
                    commands.entity(player_entity).despawn();
                }
            }
        }
    }

    while let Some(message) = client.receive_message(WORLD_SYNC_CHANNEL) {
        let world: WorldSync = bincode::deserialize(&message).unwrap();
        for (player, translation) in world.players_positions.iter() {
            if let Some(player_entity) = lobby.players.get(player) {
                let transform = Transform { translation: translation.extend(0.), ..default() };
                commands.entity(*player_entity).insert(transform);
            }
        }
    }
}

/// set up a simple 2D scene
fn setup(mut commands: Commands) {
    // camera
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
}

fn player_input(keyboard_input: Res<Input<KeyCode>>, mut player_input: ResMut<PlayerInput>) {
    player_input.left = keyboard_input.pressed(KeyCode::A) || keyboard_input.pressed(KeyCode::Left);
    player_input.right =
        keyboard_input.pressed(KeyCode::D) || keyboard_input.pressed(KeyCode::Right);
    player_input.up = keyboard_input.pressed(KeyCode::W) || keyboard_input.pressed(KeyCode::Up);
    player_input.down = keyboard_input.pressed(KeyCode::S) || keyboard_input.pressed(KeyCode::Down);
}

fn client_send_input(player_input: Res<PlayerInput>, mut client: ResMut<RenetClient>) {
    let input_message = bincode::serialize(&*player_input).unwrap();
    client.send_message(PLAYER_POSITION_CHANNEL, input_message);
}

fn camera_follow_player(
    client: Res<RenetClient>,
    lobby: Res<Lobby>,
    transforms: Query<&Transform, (With<Player>, Without<Camera>)>,
    mut cameras: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let player = Player { id: client.client_id() };
    let entity = lobby.players.get(&player).unwrap();
    for mut cam_transform in cameras.iter_mut() {
        cam_transform.translation = transforms.get(*entity).unwrap().translation;
    }
}

fn run_if_player_exist(
    client: Res<RenetClient>,
    lobby: Res<Lobby>,
    transforms: Query<&Transform, With<Player>>,
) -> ShouldRun {
    let player = Player { id: client.client_id() };
    if lobby.players.get(&player).map_or(false, |id| transforms.get(*id).is_ok()) {
        ShouldRun::Yes
    } else {
        ShouldRun::No
    }
}

/// Close the connection with the server when exiting the app.
fn close_connection_exit_system(events: EventReader<AppExit>, mut client: ResMut<RenetClient>) {
    if !events.is_empty() {
        client.disconnect();
    }
}

struct LogRttConfig {
    /// How often to display the Round-Trip time (repeating timer)
    timer: Timer,
}

/// Log the RTT in set intervals of time
fn log_rtt(time: Res<Time>, client: Res<RenetClient>, mut config: ResMut<LogRttConfig>) {
    // tick the timer
    config.timer.tick(time.delta());

    if config.timer.finished() {
        let rtt = client.network_info().rtt;
        eprintln!("UDP Round-trip time: {:0.02?}ms", rtt);
    }
}
