//! Player movement kernel (wish-vector + ground clamp + jump / swim / flight).
//!
//! Vertical state machine is aligned with upstream `src/sim/player_motion.ts`
//! (gravity, coyote jump, swim tread, fall damage). Travel flight is a rewrite
//! convenience mode (toggle) rather than a full mount/form system.

use crate::entity::Entity;
use crate::physics::{eastbrook_buildings, sweep_character_xz};
use crate::types::{
    AIR_CONTROL_ACCEL, COYOTE_TIME, FALL_SAFE_DISTANCE, FLY_SPEED_MULT, FLY_VERTICAL_SPEED,
    GRAVITY, JUMP_VELOCITY, PLAYER_RADIUS, PLAYER_SWIM_DEPTH, RUN_SPEED, SWIM_SPEED_MULT,
};
use crate::world::{
    clamp_to_world, ground_height, terrain_steepness, water_level_at, PLAYER_MAX_CLIMB_SLOPE,
    WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
};
use woc_protocol::{PlayerIntent, DT};

/// Character capsule height used for Y-overlap tests against building AABBs.
pub const PLAYER_HEIGHT: f32 = 1.8;

/// Horizontal substeps per tick — reduces slope / collider tunneling.
const MOTION_SUBSTEPS: u32 = 4;

/// Legacy absolute rise cap (yards per substep) — kept as a secondary soft gate.
pub const MAX_GROUND_STEP: f32 = 0.85;

/// Fall damage applied this tick (yards dropped past safe distance → hp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionEffect {
    pub fall_damage: f32,
}

fn clamp_to_world_padded(x: f32, z: f32) -> (f32, f32) {
    let x_limit = WORLD_MAX_X - PLAYER_RADIUS;
    (
        x.clamp(-x_limit, x_limit),
        z.clamp(WORLD_MIN_Z + PLAYER_RADIUS, WORLD_MAX_Z - PLAYER_RADIUS),
    )
}

/// Feet Y when treading a lake surface.
pub fn swim_surface_y(x: f32, z: f32) -> f32 {
    water_level_at(x, z) - 0.75
}

/// True when standing/treading in deep water over a declared lake.
pub fn is_swimming(player: &Entity) -> bool {
    let ground = ground_height(player.x, player.z, WORLD_SEED);
    let water = water_level_at(player.x, player.z);
    ground < water - PLAYER_SWIM_DEPTH && player.y <= swim_surface_y(player.x, player.z) + 0.15
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
    if terrain_steepness(next_x, next_z, WORLD_SEED) > PLAYER_MAX_CLIMB_SLOPE + 0.05 && rise > 0.05
    {
        return None;
    }
    Some(next_y)
}

fn try_move_xz(x: f32, y: f32, z: f32, dx: f32, dz: f32, grounded: bool) -> (f32, f32, f32) {
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
    if !grounded {
        // Airborne / flying / swim: keep current Y; vertical pass owns height.
        return (sx, y, sz);
    }
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

fn apply_horizontal_wish(
    player: &mut Entity,
    move_x: f32,
    move_z: f32,
    facing: f32,
    speed: f32,
    grounded: bool,
) {
    let wish_len = (move_x * move_x + move_z * move_z).sqrt();
    if wish_len < 0.01 {
        if grounded && !player.flying {
            player.vx = 0.0;
            player.vz = 0.0;
        }
        return;
    }
    let mx = move_x / wish_len;
    let mz = move_z / wish_len;
    let sin_y = facing.sin();
    let cos_y = facing.cos();
    let wish_vx = (mx * cos_y + mz * sin_y) * speed;
    let wish_vz = (-mx * sin_y + mz * cos_y) * speed;

    if grounded || player.flying || is_swimming(player) {
        player.vx = wish_vx;
        player.vz = wish_vz;
    } else {
        // Air control: accelerate toward wish, capped at wish speed.
        let ax = wish_vx - player.vx;
        let az = wish_vz - player.vz;
        let a_len = (ax * ax + az * az).sqrt();
        let step = AIR_CONTROL_ACCEL * DT;
        if a_len > 1e-4 {
            let k = (step / a_len).min(1.0);
            player.vx += ax * k;
            player.vz += az * k;
        }
        let after = (player.vx * player.vx + player.vz * player.vz).sqrt();
        let cap = speed.max((wish_vx * wish_vx + wish_vz * wish_vz).sqrt());
        if after > cap && after > 1e-6 {
            player.vx *= cap / after;
            player.vz *= cap / after;
        }
    }

    let dx = player.vx * DT;
    let dz = player.vz * DT;
    let steps = MOTION_SUBSTEPS as f32;
    let mut x = player.x;
    let mut y = player.y;
    let mut z = player.z;
    for _ in 0..MOTION_SUBSTEPS {
        let (nx, ny, nz) = try_move_xz(x, y, z, dx / steps, dz / steps, grounded && !player.flying);
        x = nx;
        y = ny;
        z = nz;
    }
    let (x, z) = clamp_to_world(x, z);
    let (x, z) = clamp_to_world_padded(x, z);
    player.x = x;
    player.z = z;
    if grounded && !player.flying {
        player.y = y;
    }
}

fn vertical_pass(player: &mut Entity, intent: &PlayerIntent, wish_speed: f32) -> Option<f32> {
    let ground = ground_height(player.x, player.z, WORLD_SEED);
    let water = water_level_at(player.x, player.z);
    let deep_water = ground < water - PLAYER_SWIM_DEPTH;
    let surface = swim_surface_y(player.x, player.z);

    // --- Travel flight -------------------------------------------------------
    if player.flying {
        if intent.jump {
            player.vy = FLY_VERTICAL_SPEED;
        } else if intent.descend {
            player.vy = -FLY_VERTICAL_SPEED;
        } else {
            player.vy = 0.0;
        }
        player.y += player.vy * DT;
        // Soft ceiling / floor.
        let min_y = ground + 0.5;
        let max_y = ground + 40.0;
        if player.y < min_y {
            player.y = min_y;
            player.vy = 0.0;
            // Touching near-ground while not ascending lands and exits flight.
            if !intent.jump {
                player.flying = false;
                player.on_ground = true;
                player.jumping = false;
                player.y = ground;
                player.fall_start_y = ground;
            }
        } else if player.y > max_y {
            player.y = max_y;
            player.vy = 0.0;
        } else {
            player.on_ground = false;
            player.jumping = false;
        }
        return None;
    }

    // --- Swim tread ----------------------------------------------------------
    if deep_water && player.y <= surface + 0.05 {
        player.y = surface;
        player.vy = 0.0;
        player.vx = 0.0;
        player.vz = 0.0;
        player.on_ground = true;
        player.jumping = false;
        player.fall_start_y = player.y;
        if intent.descend {
            // Brief dive under the surface.
            player.y = surface - 0.6;
            player.on_ground = false;
        } else if intent.jump {
            player.vy = JUMP_VELOCITY * 0.7;
            player.on_ground = false;
            player.jumping = true;
        }
        return None;
    }

    let steep_ground = player.on_ground
        && !is_swimming(player)
        && terrain_steepness(player.x, player.z, WORLD_SEED) > PLAYER_MAX_CLIMB_SLOPE;

    // Coyote: walk-off ledge still allows a jump briefly.
    let coyote = !player.on_ground
        && !player.jumping
        && !is_swimming(player)
        && player.vy <= 0.0
        && player.vy > -GRAVITY * COYOTE_TIME
        && terrain_steepness(player.x, player.z, WORLD_SEED) <= PLAYER_MAX_CLIMB_SLOPE;

    if intent.jump && (player.on_ground || coyote) && !steep_ground {
        player.vy = JUMP_VELOCITY;
        player.on_ground = false;
        player.jumping = true;
        player.fall_start_y = player.y;
        // Carry horizontal wish into the jump.
        if wish_speed > 0.01 {
            // vx/vz already set this tick from wish.
        }
    }

    if !player.on_ground {
        player.vy -= GRAVITY * DT;
        player.y += player.vy * DT;
        player.fall_start_y = player.fall_start_y.max(player.y);

        if deep_water && player.y <= surface {
            player.y = surface;
            player.vy = 0.0;
            player.vx = 0.0;
            player.vz = 0.0;
            player.on_ground = true;
            player.jumping = false;
            player.fall_start_y = player.y;
            return None;
        }

        if player.y <= ground {
            let drop = player.fall_start_y - ground;
            player.y = ground;
            player.vy = 0.0;
            player.vx = 0.0;
            player.vz = 0.0;
            player.on_ground = true;
            player.jumping = false;
            let mut dmg = 0.0;
            if drop > FALL_SAFE_DISTANCE {
                dmg = (player.hp_max * (drop - FALL_SAFE_DISTANCE) * 0.07).round();
            }
            player.fall_start_y = ground;
            return if dmg > 0.0 { Some(dmg) } else { None };
        }
        return None;
    }

    // Grounded: stick to terrain; walk off steep drops.
    let support = ground;
    let max_step_down = MAX_GROUND_STEP.max(0.4);
    if support < player.y - max_step_down {
        player.on_ground = false;
        player.jumping = false;
        player.vy = 0.0;
        player.fall_start_y = player.y;
    } else {
        player.y = support;
        player.fall_start_y = support;
        player.vy = 0.0;
    }
    None
}

/// Step one player from intent. Returns optional fall damage to apply.
pub fn step_player_motion(player: &mut Entity, intent: &PlayerIntent) -> Option<MotionEffect> {
    player.yaw = intent.facing;

    if intent.fly_toggle && player.alive {
        player.flying = !player.flying;
        if player.flying {
            player.on_ground = false;
            player.jumping = false;
            player.vy = 0.0;
            player.y = player
                .y
                .max(ground_height(player.x, player.z, WORLD_SEED) + 1.5);
            player.fall_start_y = player.y;
        } else {
            // Drop out of flight into a fall / land.
            player.vy = 0.0;
            player.fall_start_y = player.y;
            let ground = ground_height(player.x, player.z, WORLD_SEED);
            if (player.y - ground).abs() < 0.75 {
                player.y = ground;
                player.on_ground = true;
            } else {
                player.on_ground = false;
            }
        }
    }

    let swimming = is_swimming(player);
    let speed = if player.flying {
        RUN_SPEED * FLY_SPEED_MULT
    } else if swimming {
        RUN_SPEED * SWIM_SPEED_MULT
    } else {
        RUN_SPEED
    };

    let grounded = player.on_ground && !player.flying;
    apply_horizontal_wish(
        player,
        intent.move_x,
        intent.move_z,
        intent.facing,
        speed,
        grounded,
    );

    let fall = vertical_pass(player, intent, speed);

    if player.instance_id.is_none() {
        player.zone_id = crate::world::zone_band_at(player.z).id.to_string();
    }

    fall.map(|fall_damage| MotionEffect { fall_damage })
}

/// Legacy helper used by older call sites / tests that only pass wish axes.
pub fn step_player_motion_axes(player: &mut Entity, move_x: f32, move_z: f32, facing: f32) {
    let intent = PlayerIntent {
        move_x,
        move_z,
        facing,
        ..Default::default()
    };
    let _ = step_player_motion(player, &intent);
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
            step_player_motion_axes(&mut player, 0.0, 1.0, 0.0);
            let expected = ground_height(player.x, player.z, WORLD_SEED);
            assert!(
                (player.y - expected).abs() < 1e-3,
                "feet left terrain: y={} expected={}",
                player.y,
                expected
            );
            assert!(player.on_ground);
        }
    }

    #[test]
    fn ground_step_rejects_steep_rise() {
        let hub_y = ground_height(0.0, 0.0, WORLD_SEED);
        assert!(ground_step(hub_y, 0.0, 0.0, 1.0).is_some());
        assert!(ground_step(hub_y - 5.0, 0.0, 0.0, 1.0).is_none());
        let tiny = 0.2;
        let from = hub_y - (PLAYER_MAX_CLIMB_SLOPE + 0.5) * tiny;
        assert!(ground_step(from, 0.0, 0.0, tiny).is_none());
    }

    #[test]
    fn world_bounds_clamp_x() {
        let mut player = create_player(1, "Edge", PlayerClass::Warrior, WORLD_MAX_X - 0.1, 0.0);
        step_player_motion_axes(&mut player, 1.0, 0.0, 0.0);
        assert!(player.x <= WORLD_MAX_X - PLAYER_RADIUS + 1e-3);
    }

    #[test]
    fn climb_limit_matches_upstream() {
        assert!((PLAYER_MAX_CLIMB_SLOPE - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn jump_leaves_ground_and_lands() {
        let mut player = create_player(1, "Jumpy", PlayerClass::Warrior, 0.0, 0.0);
        let ground = player.y;
        let intent = PlayerIntent {
            jump: true,
            ..Default::default()
        };
        let _ = step_player_motion(&mut player, &intent);
        assert!(!player.on_ground, "jump should leave the ground");
        assert!(player.y > ground + 0.05, "should rise after jump");
        assert!(player.jumping);

        // Coast until landing (no further jump presses).
        let coast = PlayerIntent::default();
        for _ in 0..40 {
            let _ = step_player_motion(&mut player, &coast);
            if player.on_ground {
                break;
            }
        }
        assert!(player.on_ground, "should land");
        assert!((player.y - ground_height(player.x, player.z, WORLD_SEED)).abs() < 1e-2);
        assert!(!player.jumping);
    }

    #[test]
    fn fly_toggle_enables_vertical_ascend() {
        let mut player = create_player(1, "Flyer", PlayerClass::Mage, 0.0, 0.0);
        let start_y = player.y;
        let toggle = PlayerIntent {
            fly_toggle: true,
            ..Default::default()
        };
        let _ = step_player_motion(&mut player, &toggle);
        assert!(player.flying);
        let up = PlayerIntent {
            jump: true,
            ..Default::default()
        };
        for _ in 0..10 {
            let _ = step_player_motion(&mut player, &up);
        }
        assert!(
            player.y > start_y + 2.0,
            "flight ascend should gain altitude"
        );
    }

    #[test]
    fn long_fall_reports_damage() {
        let mut player = create_player(1, "Cliff", PlayerClass::Warrior, 0.0, 0.0);
        player.on_ground = false;
        player.jumping = false;
        player.y = player.y + 25.0;
        player.fall_start_y = player.y;
        player.vy = 0.0;
        let mut hit = None;
        for _ in 0..80 {
            hit = step_player_motion(&mut player, &PlayerIntent::default());
            if player.on_ground {
                break;
            }
        }
        assert!(player.on_ground);
        assert!(
            hit.map(|e| e.fall_damage > 0.0).unwrap_or(false),
            "25yd fall should deal damage"
        );
    }
}
