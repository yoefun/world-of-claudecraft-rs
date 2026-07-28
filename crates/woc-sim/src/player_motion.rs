//! Player movement kernel (wish-vector + ground clamp + climb slope + buildings).

use crate::entity::Entity;
use crate::physics::{eastbrook_buildings, sweep_character_xz};
use crate::types::{PLAYER_RADIUS, RUN_SPEED};
use crate::world::{
    clamp_to_world, ground_height, terrain_steepness, PLAYER_MAX_CLIMB_SLOPE, WORLD_MAX_X,
    WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
};
use woc_protocol::DT;

/// Character capsule height used for Y-overlap tests against building AABBs.
pub const PLAYER_HEIGHT: f32 = 1.8;

/// Horizontal substeps per tick — reduces slope / collider tunneling.
const MOTION_SUBSTEPS: u32 = 4;

/// Legacy absolute rise cap (yards per substep) — kept as a secondary soft gate.
pub const MAX_GROUND_STEP: f32 = 0.85;

fn clamp_to_world_padded(x: f32, z: f32) -> (f32, f32) {
    let x_limit = WORLD_MAX_X - PLAYER_RADIUS;
    (
        x.clamp(-x_limit, x_limit),
        z.clamp(WORLD_MIN_Z + PLAYER_RADIUS, WORLD_MAX_Z - PLAYER_RADIUS),
    )
}

/// Accept a proposed ground sample only if climb slope / rise is walkable.
pub fn ground_step(prev_y: f32, next_x: f32, next_z: f32, horiz: f32) -> Option<f32> {
    let next_y = ground_height(next_x, next_z, WORLD_SEED);
    let rise = next_y - prev_y;
    if rise > MAX_GROUND_STEP {
        return None;
    }
    if horiz > 1e-4 && rise / horiz > PLAYER_MAX_CLIMB_SLOPE {
        return None;
    }
    // Reject stepping onto locally impassable wall cells.
    if terrain_steepness(next_x, next_z, WORLD_SEED) > PLAYER_MAX_CLIMB_SLOPE + 0.05 && rise > 0.05
    {
        return None;
    }
    Some(next_y)
}

fn try_move_xz(x: f32, y: f32, z: f32, dx: f32, dz: f32) -> (f32, f32, f32) {
    let horiz = (dx * dx + dz * dz).sqrt();
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
    if let Some(ny) = ground_step(y, sx, sz, horiz) {
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
    if let Some(ny) = ground_step(y, sx_only, sz_keep, dx.abs()) {
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
    if let Some(ny) = ground_step(y, sx_keep, sz_only, dz.abs()) {
        return (sx_keep, ny, sz_only);
    }
    (x, ground_height(x, z, WORLD_SEED), z)
}

pub fn step_player_motion(player: &mut Entity, move_x: f32, move_z: f32, facing: f32) {
    player.yaw = facing;
    let wish_len = (move_x * move_x + move_z * move_z).sqrt();
    if wish_len < 0.01 {
        player.y = ground_height(player.x, player.z, WORLD_SEED);
        return;
    }
    let mx = move_x / wish_len;
    let mz = move_z / wish_len;

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

    let (x, z) = clamp_to_world(x, z);
    let (x, z) = clamp_to_world_padded(x, z);
    player.x = x;
    player.z = z;
    player.y = ground_height(player.x, player.z, WORLD_SEED);
    // Keep zone_id coherent with strip position when overworld.
    if player.instance_id.is_none() {
        player.zone_id = crate::world::zone_band_at(player.z).id.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn slope_following_keeps_feet_on_terrain() {
        let mut player = create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        for _ in 0..40 {
            step_player_motion(&mut player, 0.0, 1.0, 0.0);
            let expected = ground_height(player.x, player.z, WORLD_SEED);
            assert!(
                (player.y - expected).abs() < 1e-3,
                "feet left terrain: y={} expected={}",
                player.y,
                expected
            );
        }
    }

    #[test]
    fn ground_step_rejects_steep_rise() {
        let hub_y = ground_height(0.0, 0.0, WORLD_SEED);
        // Standing on the hub plateau is walkable for a one-yard step.
        assert!(
            ground_step(hub_y, 0.0, 0.0, 1.0).is_some(),
            "hub plateau should accept flat step from feet height {hub_y}"
        );
        // Large absolute rise from below the surface fails the soft step cap.
        assert!(
            ground_step(hub_y - 5.0, 0.0, 0.0, 1.0).is_none(),
            "rising 5yd onto hub must be rejected"
        );
        // Rise/run above climb limit (even when under absolute step) is blocked.
        let tiny = 0.2;
        let from = hub_y - (PLAYER_MAX_CLIMB_SLOPE + 0.5) * tiny;
        assert!(
            ground_step(from, 0.0, 0.0, tiny).is_none(),
            "slope above climb limit must reject"
        );
    }

    #[test]
    fn world_bounds_clamp_x() {
        let mut player = create_player(1, "Edge", PlayerClass::Warrior, WORLD_MAX_X - 0.1, 0.0);
        step_player_motion(&mut player, 1.0, 0.0, 0.0);
        assert!(
            player.x <= WORLD_MAX_X - PLAYER_RADIUS + 1e-3,
            "escaped x bound {}",
            player.x
        );
    }

    #[test]
    fn climb_limit_matches_upstream() {
        assert!((PLAYER_MAX_CLIMB_SLOPE - 1.5).abs() < f32::EPSILON);
    }
}
