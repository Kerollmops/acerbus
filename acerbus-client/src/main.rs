use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, SystemTime};

use acerbus_common::{panic_on_error_system, Lobby, PlayerInput, ServerMessage, PROTOCOL_ID};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_asset_loader::{AssetCollection, AssetCollectionApp};
use bevy_renet::renet::{ClientAuthentication, RenetClient, RenetConnectionConfig};
use bevy_renet::{run_if_client_conected, RenetClientPlugin};

#[derive(AssetCollection)]
struct GameAssets {
    #[asset(path = "images/icon-green.png")]
    icon_green: Handle<Image>,
    #[asset(path = "images/icon-purple.png")]
    icon_purple: Handle<Image>,
}

fn new_renet_client() -> RenetClient {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
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

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.init_collection::<GameAssets>();
    app.insert_resource(Lobby::default());

    app.add_plugin(RenetClientPlugin);
    app.insert_resource(new_renet_client());
    app.insert_resource(PlayerInput::default());
    app.add_system(player_input);
    app.add_system(client_send_input.with_run_criteria(run_if_client_conected));
    app.add_system(client_sync_players.with_run_criteria(run_if_client_conected));

    app.insert_resource(LogRttConfig { timer: Timer::new(Duration::from_secs(5), true) });
    app.add_system(log_rtt.with_run_criteria(run_if_client_conected));

    app.add_startup_system(setup);
    app.add_system_to_stage(CoreStage::PostUpdate, close_connection_exit_system);
    app.add_system(panic_on_error_system);

    app.run();
}

fn client_sync_players(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut client: ResMut<RenetClient>,
    mut lobby: ResMut<Lobby>,
) {
    while let Some(message) = client.receive_message(0) {
        let server_message = bincode::deserialize(&message).unwrap();
        match server_message {
            ServerMessage::PlayerConnected { id } => {
                println!("Player {} connected.", id);
                let player_entity = commands
                    .spawn_bundle(SpriteBundle {
                        texture: game_assets.icon_green.clone(),
                        ..default()
                    })
                    .id();

                lobby.players.insert(id, player_entity);
            }
            ServerMessage::PlayerDisconnected { id } => {
                println!("Player {} disconnected.", id);
                if let Some(player_entity) = lobby.players.remove(&id) {
                    commands.entity(player_entity).despawn();
                }
            }
        }
    }

    while let Some(message) = client.receive_message(1) {
        let players: HashMap<u64, [f32; 3]> = bincode::deserialize(&message).unwrap();
        for (player_id, translation) in players.iter() {
            if let Some(player_entity) = lobby.players.get(player_id) {
                let transform = Transform { translation: (*translation).into(), ..default() };
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
    client.send_message(0, input_message);
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
