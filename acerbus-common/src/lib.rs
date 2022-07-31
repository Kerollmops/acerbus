use std::collections::HashMap;

use bevy::prelude::*;
use bevy_renet::renet::RenetError;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_ID: u64 = 7;

pub const PLAYER_MOVE_SPEED: f32 = 100.0;
pub const PLAYER_SQUARE_SIZE: f32 = 50.0;

pub const PLAYER_POSITION_CHANNEL: u8 = 0;
pub const CONNECTION_EVENTS_CHANNEL: u8 = 0;
pub const WORLD_SYNC_CHANNEL: u8 = 1;

#[derive(Debug, Default, Serialize, Deserialize, Component)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Serialize, Deserialize)]
pub struct Player {
    pub id: u64,
}

#[derive(Debug, Default)]
pub struct Lobby {
    pub players: HashMap<Player, Entity>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct WorldSync {
    pub players_positions: HashMap<Player, Vec2>,
}

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessage {
    PlayerConnected { player: Player },
    PlayerDisconnected { player: Player },
}

// If any error is found we just panic
pub fn panic_on_error_system(mut renet_error: EventReader<RenetError>) {
    for e in renet_error.iter() {
        panic!("{}", e);
    }
}
