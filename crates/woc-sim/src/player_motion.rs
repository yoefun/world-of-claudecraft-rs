//! Player movement kernel (wish-vector + ground clamp + building sweep).

use crate::entity::Entity;
use crate::physics::{eastbrook_buildings, sweep_character_xz};
use crate::types::{PLAYER_RADIUS, RUN_SPEED};
use crate::world::{clamp_to_world, terrain_height, WORLD_HALF, WORLD_SEED};
use woc_protocol::DT;

/// Character capsule height used for Y-overlap tests against building AABBs.
pub const PLAYER_HEIGHT: f32 = 1.8;

/// Horizontal substeps per tick — reduces slope / collider tunneling.
const MOTION_SUBSTEPS: u32 = 4;

/// Max terrain rise accepted in one substep (yards). Steeper = blocked / slide.
pub const MAX_GROUND_STEP: f32 = 0.55;

/// World-bounds pad so the player radius stays inside the playable area.
fn clamp_to_world_padded(x: f32, z: f32) -> (f32, f32) {
    let limit = WORLD_HALF - PLAYER_RADIUS;
    (x.clamp(-limit, limit), z.clamp(-limit, limit))
}

/// Accept a proposed ground sample only if the rise from `prev_y` is walkable.
pub fn ground_step(prev_y: f32, next_x: f32, next_z: f32) -> Option<f32> {
    let next_y = terrain_height(next_x, next_z, WORLD_SEED);
    if next_y - prev_y > MAX_GROUND_STEP {
        None
    } else {
        Some(next_y)
    }
}

fn try_move_xz(x: f32, y: f32, z: f32, dx: f32, dz: f32) -> (f32, f32, f32) {
    let (sx, sz) = sweep_character_xz(
        x,
        y,
        z,
        dx,
        dz,
        PLAYER_RADIUS,
        PLAYER_HEIGHT,
        eastbrook_buildings(),
    );
    let (sx, sz) = clamp_to_world_padded(sx, sz);
    // Prefer full XZ; on steep rise, try axis-separated slides.
    if let Some(ny) = ground_step(y, sx, sz) {
        return (sx, ny, sz);
    }
    let (sx_only, _) = sweep_character_xz(
        x,
        y,
        z,
        dx,
        0.0,
        PLAYER_RADIUS,
        PLAYER_HEIGHT,
        eastbrook_buildings(),
    );
    let (sx_only, sz_keep) = clamp_to_world_padded(sx_only, z);
    if let Some(ny) = ground_step(y, sx_only, sz_keep) {
        return (sx_only, ny, sz_keep);
    }
    let (_, sz_only) = sweep_character_xz(
        x,
        y,
        z,
        0.0,
        dz,
        PLAYER_RADIUS,
        PLAYER_HEIGHT,
        eastbrook_buildings(),
    );
    let (sx_keep, sz_only) = clamp_to_world_padded(x, sz_only);
    if let Some(ny) = ground_step(y, sx_keep, sz_only) {
        return (sx_keep, ny, sz_only);
    }
    // Fully blocked — stay put but still snap to current ground.
    (x, terrain_height(x, z, WORLD_SEED), z)
}

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

    let steps = MOTION_SUBSTEPS as f32;
    let mut x = player.x;
    let mut y = player.y;
    let mut z = player.z;
    for _ in 0..MOTION_SUBSTEPS {
        let (nx, ny, nz) = try_move_xz(x, y, z, dx / steps, dz / steps);
        x = nx;
        y = ny;
        z = nz;
    }

    // Final safety: never leave the world extent (legacy helper kept for parity).
    let (x, z) = clamp_to_world(x, z);
    let (x, z) = clamp_to_world_padded(x, z);
    player.x = x;
    player.z = z;
    player.y = terrain_height(player.x, player.z, WORLD_SEED);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use crate::physics::{eastbrook_buildings, EASTBROOK_INN};
    use woc_content::PlayerClass;

    fn inside_inn_xz(x: f32, z: f32) -> bool {
        let b = EASTBROOK_INN;
        x + PLAYER_RADIUS > b.min_x
            && x - PLAYER_RADIUS < b.max_x
            && z + PLAYER_RADIUS > b.min_z
            && z - PLAYER_RADIUS < b.max_z
    }

    #[test]
    fn eastbrook_inn_blocks_player_motion() {
        // Start just south of the inn; walk due north (facing 0, move_z = 1).
        let mut player = create_player(1, "Walker", PlayerClass::Warrior, 1.0, 9.0);
        assert!(
            !inside_inn_xz(player.x, player.z),
            "test setup must start outside the inn"
        );
        for _ in 0..80 {
            step_player_motion(&mut player, 0.0, 1.0, 0.0);
        }
        assert!(
            !inside_inn_xz(player.x, player.z),
            "player penetrated Eastbrook Inn AABB at ({}, {})",
            player.x,
            player.z
        );
        assert!(
            player.z <= EASTBROOK_INN.min_z - PLAYER_RADIUS + 0.05,
            "expected to stop at inn south face, z={}",
            player.z
        );
        assert!(!eastbrook_buildings().is_empty());
    }

    #[test]
    fn same_intents_same_positions() {
        let intents = [
            (0.0, 1.0, 0.0),
            (1.0, 1.0, 0.4),
            (-0.5, 1.0, -0.2),
            (0.0, 1.0, 1.2),
            (1.0, 0.0, 0.0),
            (-1.0, 0.3, 2.0),
            (0.2, -1.0, -0.5),
            (0.0, 1.0, 0.1),
        ];
        let mut a = create_player(1, "A", PlayerClass::Warrior, 2.0, 4.0);
        let mut b = create_player(1, "A", PlayerClass::Warrior, 2.0, 4.0);
        for _ in 0..25 {
            for &(mx, mz, facing) in &intents {
                step_player_motion(&mut a, mx, mz, facing);
                step_player_motion(&mut b, mx, mz, facing);
            }
        }
        assert_eq!(a.x.to_bits(), b.x.to_bits());
        assert_eq!(a.y.to_bits(), b.y.to_bits());
        assert_eq!(a.z.to_bits(), b.z.to_bits());
        assert_eq!(a.yaw.to_bits(), b.yaw.to_bits());
    }

    #[test]
    fn slope_following_keeps_feet_on_terrain() {
        let mut player = create_player(1, "Climber", PlayerClass::Warrior, -10.0, -10.0);
        for i in 0..60 {
            let facing = (i as f32) * 0.15;
            step_player_motion(&mut player, 0.3, 1.0, facing);
            let expected = terrain_height(player.x, player.z, WORLD_SEED);
            assert!(
                (player.y - expected).abs() < 1e-4,
                "feet left terrain: y={} expected={}",
                player.y,
                expected
            );
        }
    }

    #[test]
    fn ground_step_rejects_steep_rise() {
        // Pick a real sample, then pretend we arrived from far below so the rise
        // exceeds MAX_GROUND_STEP — the clamp must refuse the step.
        let x = 5.0;
        let z = -15.0;
        let next_y = terrain_height(x, z, WORLD_SEED);
        let prev_y = next_y - MAX_GROUND_STEP - 0.5;
        assert!(ground_step(prev_y, x, z).is_none());
        // From a nearly-matching height, the same cell is walkable.
        assert_eq!(ground_step(next_y - 0.1, x, z), Some(next_y));
    }

    #[test]
    fn world_clamp_respects_player_radius() {
        let mut player = create_player(1, "Edge", PlayerClass::Warrior, WORLD_HALF - 0.1, 0.0);
        for _ in 0..30 {
            step_player_motion(&mut player, 1.0, 0.0, 0.0);
        }
        assert!(
            player.x <= WORLD_HALF - PLAYER_RADIUS + 1e-3,
            "player radius poked past world edge: x={}",
            player.x
        );
    }
}
