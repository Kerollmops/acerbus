use std::collections::HashMap;

use bevy::prelude::*;
use bevy_renet::renet::RenetError;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_ID: u64 = 7;

pub const PLAYER_MOVE_SPEED: f32 = 100.0;

#[derive(Debug, Default, Serialize, Deserialize, Component)]
pub struct PlayerInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Debug, Component)]
pub struct Player {
    pub id: u64,
}

#[derive(Debug, Default)]
pub struct Lobby {
    pub players: HashMap<u64, Entity>,
}

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessage {
    PlayerConnected { id: u64 },
    PlayerDisconnected { id: u64 },
}

// If any error is found we just panic
pub fn panic_on_error_system(mut renet_error: EventReader<RenetError>) {
    for e in renet_error.iter() {
        panic!("{}", e);
    }
}
