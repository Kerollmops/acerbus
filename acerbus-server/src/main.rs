use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, SystemTime};

use acerbus_common::{
    panic_on_error_system, Lobby, Player, PlayerInput, ServerMessage, PLAYER_MOVE_SPEED,
    PROTOCOL_ID,
};
use bevy::app::ScheduleRunnerSettings;
use bevy::prelude::*;
use bevy_renet::renet::{
    RenetConnectionConfig, RenetServer, ServerAuthentication, ServerConfig, ServerEvent,
};
use bevy_renet::RenetServerPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(ScheduleRunnerSettings::run_loop(Duration::from_secs_f64(1.0 / 60.0)));

    app.insert_resource(Lobby::default());

    app.add_plugin(RenetServerPlugin);
    app.insert_resource(new_renet_server());
    app.add_system(server_update_system);
    app.add_system(server_sync_players);
    app.add_system(move_players_system);

    app.add_startup_system(setup);
    app.add_system(panic_on_error_system);

    app.run();
}

fn setup(_commands: Commands) {}

fn new_renet_server() -> RenetServer {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let socket = UdpSocket::bind(server_addr).unwrap();
    let connection_config = RenetConnectionConfig::default();
    let server_config =
        ServerConfig::new(64, PROTOCOL_ID, server_addr, ServerAuthentication::Unsecure);
    let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    RenetServer::new(current_time, server_config, connection_config, socket).unwrap()
}

fn server_update_system(
    mut server_events: EventReader<ServerEvent>,
    mut commands: Commands,
    mut lobby: ResMut<Lobby>,
    mut server: ResMut<RenetServer>,
) {
    for event in server_events.iter() {
        match event {
            ServerEvent::ClientConnected(id, _) => {
                println!("Player {} connected.", id);
                // Spawn player cube
                let player_entity = commands
                    .spawn()
                    .insert(Transform::default())
                    .insert(GlobalTransform::default())
                    .insert(PlayerInput::default())
                    .insert(Player { id: *id })
                    .id();

                // We could send an InitState with all the players id and positions for the client
                // but this is easier to do.
                for &player_id in lobby.players.keys() {
                    let message =
                        bincode::serialize(&ServerMessage::PlayerConnected { id: player_id })
                            .unwrap();
                    server.send_message(*id, 0, message);
                }

                lobby.players.insert(*id, player_entity);

                let message =
                    bincode::serialize(&ServerMessage::PlayerConnected { id: *id }).unwrap();
                server.broadcast_message(0, message);
            }
            ServerEvent::ClientDisconnected(id) => {
                println!("Player {} disconnected.", id);
                if let Some(player_entity) = lobby.players.remove(id) {
                    commands.entity(player_entity).despawn();
                }

                let message =
                    bincode::serialize(&ServerMessage::PlayerDisconnected { id: *id }).unwrap();
                server.broadcast_message(0, message);
            }
        }
    }

    // We move the players on the server side
    for client_id in server.clients_id().into_iter() {
        while let Some(message) = server.receive_message(client_id, 0) {
            let player_input: PlayerInput = bincode::deserialize(&message).unwrap();
            if let Some(player_entity) = lobby.players.get(&client_id) {
                commands.entity(*player_entity).insert(player_input);
            }
        }
    }
}

fn server_sync_players(mut server: ResMut<RenetServer>, query: Query<(&Transform, &Player)>) {
    let mut players: HashMap<u64, [f32; 3]> = HashMap::new();
    for (transform, player) in query.iter() {
        players.insert(player.id, transform.translation.into());
    }

    let sync_message = bincode::serialize(&players).unwrap();
    server.broadcast_message(1, sync_message);
}

fn move_players_system(mut query: Query<(&mut Transform, &PlayerInput)>, time: Res<Time>) {
    for (mut transform, input) in query.iter_mut() {
        let x = (input.right as i8 - input.left as i8) as f32;
        let y = (input.up as i8 - input.down as i8) as f32;
        transform.translation.x += x * PLAYER_MOVE_SPEED * time.delta().as_secs_f32();
        transform.translation.y += y * PLAYER_MOVE_SPEED * time.delta().as_secs_f32();
    }
}
