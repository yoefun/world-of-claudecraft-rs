//! Player movement kernel (simplified wish-vector + ground clamp).

use crate::entity::Entity;
use crate::types::{PLAYER_RADIUS, RUN_SPEED};
use crate::world::{clamp_to_world, terrain_height, WORLD_SEED};
use woc_protocol::DT;

pub fn step_player_motion(player: &mut Entity, move_x: f32, move_z: f32, facing: f32) {
    player.yaw = facing;
    let wish_len = (move_x * move_x + move_z * move_z).sqrt();
    if wish_len < 0.01 {
        player.y = terrain_height(player.x, player.z, WORLD_SEED);
        return;
    }
    let mx = move_x / wish_len;
    let mz = move_z / wish_len;

    // Camera-relative wish: move_z is forward along facing, move_x is strafe.
    let sin_y = facing.sin();
    let cos_y = facing.cos();
    let dx = (mx * cos_y + mz * sin_y) * RUN_SPEED * DT;
    let dz = (-mx * sin_y + mz * cos_y) * RUN_SPEED * DT;

    let (nx, nz) = clamp_to_world(player.x + dx, player.z + dz);
    // Soft collision against world bounds already applied; keep a tiny radius pad.
    let _ = PLAYER_RADIUS;
    player.x = nx;
    player.z = nz;
    player.y = terrain_height(player.x, player.z, WORLD_SEED);
}
