//! Player movement kernel (wish-vector + ground clamp + jump / swim / flight).
//!
//! Vertical state machine is aligned with upstream `src/sim/player_motion.ts`
//! (gravity, coyote jump, swim tread, fall damage). Travel flight is a rewrite
//! convenience mode (toggle) rather than a full mount/form system.

use crate::ecs::components::{Health, Identity, InstanceAt, Motion, Transform};
use crate::ecs::World;
use crate::physics::{eastbrook_buildings, sweep_character_xz};
use crate::types::{
    AIR_CONTROL_ACCEL, COYOTE_TIME, FALL_SAFE_DISTANCE, FLY_SPEED_MULT, FLY_VERTICAL_SPEED,
    GRAVITY, JUMP_VELOCITY, PLAYER_RADIUS, PLAYER_SWIM_DEPTH, RUN_SPEED, SWIM_SPEED_MULT,
};
use crate::world::{
    clamp_to_world, ground_height, terrain_steepness, water_level_at, PLAYER_MAX_CLIMB_SLOPE,
    WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
};
use woc_protocol::{EntityId, PlayerIntent, DT};

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
pub fn is_swimming_at(x: f32, y: f32, z: f32) -> bool {
    let ground = ground_height(x, z, WORLD_SEED);
    let water = water_level_at(x, z);
    ground < water - PLAYER_SWIM_DEPTH && y <= swim_surface_y(x, z) + 0.15
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
    t: &mut Transform,
    m: &mut Motion,
    move_x: f32,
    move_z: f32,
    facing: f32,
    speed: f32,
    grounded: bool,
) {
    let wish_len = (move_x * move_x + move_z * move_z).sqrt();
    if wish_len < 0.01 {
        if grounded && !m.flying {
            m.vx = 0.0;
            m.vz = 0.0;
        }
        return;
    }
    let mx = move_x / wish_len;
    let mz = move_z / wish_len;
    let sin_y = facing.sin();
    let cos_y = facing.cos();
    let wish_vx = (mx * cos_y + mz * sin_y) * speed;
    let wish_vz = (-mx * sin_y + mz * cos_y) * speed;

    if grounded || m.flying || is_swimming_at(t.x, t.y, t.z) {
        m.vx = wish_vx;
        m.vz = wish_vz;
    } else {
        // Air control: accelerate toward wish, capped at wish speed.
        let ax = wish_vx - m.vx;
        let az = wish_vz - m.vz;
        let a_len = (ax * ax + az * az).sqrt();
        let step = AIR_CONTROL_ACCEL * DT;
        if a_len > 1e-4 {
            let k = (step / a_len).min(1.0);
            m.vx += ax * k;
            m.vz += az * k;
        }
        let after = (m.vx * m.vx + m.vz * m.vz).sqrt();
        let cap = speed.max((wish_vx * wish_vx + wish_vz * wish_vz).sqrt());
        if after > cap && after > 1e-6 {
            m.vx *= cap / after;
            m.vz *= cap / after;
        }
    }

    let dx = m.vx * DT;
    let dz = m.vz * DT;
    let steps = MOTION_SUBSTEPS as f32;
    let mut x = t.x;
    let mut y = t.y;
    let mut z = t.z;
    for _ in 0..MOTION_SUBSTEPS {
        let (nx, ny, nz) = try_move_xz(x, y, z, dx / steps, dz / steps, grounded && !m.flying);
        x = nx;
        y = ny;
        z = nz;
    }
    let (x, z) = clamp_to_world(x, z);
    let (x, z) = clamp_to_world_padded(x, z);
    t.x = x;
    t.z = z;
    if grounded && !m.flying {
        t.y = y;
    }
}

fn vertical_pass(
    t: &mut Transform,
    m: &mut Motion,
    hp_max: f32,
    intent: &PlayerIntent,
    wish_speed: f32,
) -> Option<f32> {
    let ground = ground_height(t.x, t.z, WORLD_SEED);
    let water = water_level_at(t.x, t.z);
    let deep_water = ground < water - PLAYER_SWIM_DEPTH;
    let surface = swim_surface_y(t.x, t.z);

    // --- Travel flight -------------------------------------------------------
    if m.flying {
        if intent.jump {
            m.vy = FLY_VERTICAL_SPEED;
        } else if intent.descend {
            m.vy = -FLY_VERTICAL_SPEED;
        } else {
            m.vy = 0.0;
        }
        t.y += m.vy * DT;
        // Soft ceiling / floor.
        let min_y = ground + 0.5;
        let max_y = ground + 40.0;
        if t.y < min_y {
            t.y = min_y;
            m.vy = 0.0;
            // Touching near-ground while not ascending lands and exits flight.
            if !intent.jump {
                m.flying = false;
                m.on_ground = true;
                m.jumping = false;
                t.y = ground;
                m.fall_start_y = ground;
            }
        } else if t.y > max_y {
            t.y = max_y;
            m.vy = 0.0;
        } else {
            m.on_ground = false;
            m.jumping = false;
        }
        return None;
    }

    // --- Swim tread ----------------------------------------------------------
    if deep_water && t.y <= surface + 0.05 {
        t.y = surface;
        m.vy = 0.0;
        m.vx = 0.0;
        m.vz = 0.0;
        m.on_ground = true;
        m.jumping = false;
        m.fall_start_y = t.y;
        if intent.descend {
            // Brief dive under the surface.
            t.y = surface - 0.6;
            m.on_ground = false;
        } else if intent.jump {
            m.vy = JUMP_VELOCITY * 0.7;
            m.on_ground = false;
            m.jumping = true;
        }
        return None;
    }

    let steep_ground = m.on_ground
        && !is_swimming_at(t.x, t.y, t.z)
        && terrain_steepness(t.x, t.z, WORLD_SEED) > PLAYER_MAX_CLIMB_SLOPE;

    // Coyote: walk-off ledge still allows a jump briefly.
    let coyote = !m.on_ground
        && !m.jumping
        && !is_swimming_at(t.x, t.y, t.z)
        && m.vy <= 0.0
        && m.vy > -GRAVITY * COYOTE_TIME
        && terrain_steepness(t.x, t.z, WORLD_SEED) <= PLAYER_MAX_CLIMB_SLOPE;

    if intent.jump && (m.on_ground || coyote) && !steep_ground {
        m.vy = JUMP_VELOCITY;
        m.on_ground = false;
        m.jumping = true;
        m.fall_start_y = t.y;
        // Carry horizontal wish into the jump.
        if wish_speed > 0.01 {
            // vx/vz already set this tick from wish.
        }
    }

    if !m.on_ground {
        m.vy -= GRAVITY * DT;
        t.y += m.vy * DT;
        m.fall_start_y = m.fall_start_y.max(t.y);

        if deep_water && t.y <= surface {
            t.y = surface;
            m.vy = 0.0;
            m.vx = 0.0;
            m.vz = 0.0;
            m.on_ground = true;
            m.jumping = false;
            m.fall_start_y = t.y;
            return None;
        }

        if t.y <= ground {
            let drop = m.fall_start_y - ground;
            t.y = ground;
            m.vy = 0.0;
            m.vx = 0.0;
            m.vz = 0.0;
            m.on_ground = true;
            m.jumping = false;
            let mut dmg = 0.0;
            if drop > FALL_SAFE_DISTANCE {
                dmg = (hp_max * (drop - FALL_SAFE_DISTANCE) * 0.07).round();
            }
            m.fall_start_y = ground;
            return if dmg > 0.0 { Some(dmg) } else { None };
        }
        return None;
    }

    // Grounded: stick to terrain; walk off steep drops.
    let support = ground;
    let max_step_down = MAX_GROUND_STEP.max(0.4);
    if support < t.y - max_step_down {
        m.on_ground = false;
        m.jumping = false;
        m.vy = 0.0;
        m.fall_start_y = t.y;
    } else {
        t.y = support;
        m.fall_start_y = support;
        m.vy = 0.0;
    }
    None
}

/// Step one player from intent. Returns optional fall damage to apply.
pub fn step_player_motion(
    world: &mut World,
    player_id: EntityId,
    intent: &PlayerIntent,
) -> Option<MotionEffect> {
    let mut t = world.get::<Transform>(player_id).cloned()?;
    let mut m = world.get::<Motion>(player_id).cloned()?;
    let health = world.get::<Health>(player_id).cloned()?;
    let in_instance = world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.clone())
        .is_some();

    let mut intent = *intent;
    if crate::combat::is_stunned(world, player_id) {
        intent.move_x = 0.0;
        intent.move_z = 0.0;
        intent.jump = false;
        intent.descend = false;
        intent.fly_toggle = false;
    }

    t.yaw = intent.facing;

    if intent.fly_toggle && health.alive {
        m.flying = !m.flying;
        if m.flying {
            m.on_ground = false;
            m.jumping = false;
            m.vy = 0.0;
            t.y = t.y.max(ground_height(t.x, t.z, WORLD_SEED) + 1.5);
            m.fall_start_y = t.y;
        } else {
            // Drop out of flight into a fall / land.
            m.vy = 0.0;
            m.fall_start_y = t.y;
            let ground = ground_height(t.x, t.z, WORLD_SEED);
            if (t.y - ground).abs() < 0.75 {
                t.y = ground;
                m.on_ground = true;
            } else {
                m.on_ground = false;
            }
        }
    }

    let swimming = is_swimming_at(t.x, t.y, t.z);
    let speed = if m.flying {
        RUN_SPEED * FLY_SPEED_MULT
    } else if swimming {
        RUN_SPEED * SWIM_SPEED_MULT
    } else {
        RUN_SPEED
    } * crate::combat::move_speed_mult(world, player_id);

    let grounded = m.on_ground && !m.flying;
    apply_horizontal_wish(
        &mut t,
        &mut m,
        intent.move_x,
        intent.move_z,
        intent.facing,
        speed,
        grounded,
    );

    let fall = vertical_pass(&mut t, &mut m, health.hp_max, &intent, speed);

    if !in_instance {
        if let Some(ident) = world.get_mut::<Identity>(player_id) {
            ident.zone_id = crate::world::zone_band_at(t.z).id.to_string();
        }
    }
    if let Some(slot) = world.get_mut::<Transform>(player_id) {
        *slot = t;
    }
    if let Some(slot) = world.get_mut::<Motion>(player_id) {
        *slot = m;
    }

    fall.map(|fall_damage| MotionEffect { fall_damage })
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::PlayerClass;

    fn step_axes(world: &mut World, player_id: EntityId, move_x: f32, move_z: f32, facing: f32) {
        let intent = PlayerIntent {
            move_x,
            move_z,
            facing,
            ..Default::default()
        };
        let _ = step_player_motion(world, player_id, &intent);
    }

    fn player_at(x: f32, z: f32) -> World {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, x, z);
        world
    }

    #[test]
    fn slope_following_keeps_feet_on_terrain() {
        let mut world = player_at(0.0, 0.0);
        for _ in 0..40 {
            step_axes(&mut world, 1, 0.0, 1.0, 0.0);
            let t = world.get::<Transform>(1).unwrap();
            let expected = ground_height(t.x, t.z, WORLD_SEED);
            assert!((t.y - expected).abs() < 1e-3);
            assert!(world.get::<Motion>(1).unwrap().on_ground);
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
        let mut world = player_at(WORLD_MAX_X - 0.1, 0.0);
        step_axes(&mut world, 1, 1.0, 0.0, 0.0);
        assert!(world.get::<Transform>(1).unwrap().x <= WORLD_MAX_X - PLAYER_RADIUS + 1e-3);
    }

    #[test]
    fn climb_limit_matches_upstream() {
        assert!((PLAYER_MAX_CLIMB_SLOPE - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn jump_leaves_ground_and_lands() {
        let mut world = player_at(0.0, 0.0);
        let ground = world.get::<Transform>(1).unwrap().y;
        let intent = PlayerIntent {
            jump: true,
            ..Default::default()
        };
        let _ = step_player_motion(&mut world, 1, &intent);
        assert!(!world.get::<Motion>(1).unwrap().on_ground);
        assert!(world.get::<Transform>(1).unwrap().y > ground + 0.05);
        assert!(world.get::<Motion>(1).unwrap().jumping);
        let coast = PlayerIntent::default();
        for _ in 0..40 {
            let _ = step_player_motion(&mut world, 1, &coast);
            if world.get::<Motion>(1).unwrap().on_ground {
                break;
            }
        }
        assert!(world.get::<Motion>(1).unwrap().on_ground);
        let t = world.get::<Transform>(1).unwrap();
        assert!((t.y - ground_height(t.x, t.z, WORLD_SEED)).abs() < 1e-2);
        assert!(!world.get::<Motion>(1).unwrap().jumping);
    }

    #[test]
    fn fly_toggle_enables_vertical_ascend() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Flyer", PlayerClass::Mage, 0.0, 0.0);
        let start_y = world.get::<Transform>(1).unwrap().y;
        let toggle = PlayerIntent {
            fly_toggle: true,
            ..Default::default()
        };
        let _ = step_player_motion(&mut world, 1, &toggle);
        assert!(world.get::<Motion>(1).unwrap().flying);
        let up = PlayerIntent {
            jump: true,
            ..Default::default()
        };
        for _ in 0..10 {
            let _ = step_player_motion(&mut world, 1, &up);
        }
        assert!(world.get::<Transform>(1).unwrap().y > start_y + 2.0);
    }

    #[test]
    fn long_fall_reports_damage() {
        let mut world = player_at(0.0, 0.0);
        {
            let y = world.get::<Transform>(1).unwrap().y + 25.0;
            world.get_mut::<Transform>(1).unwrap().y = y;
            let m = world.get_mut::<Motion>(1).unwrap();
            m.on_ground = false;
            m.jumping = false;
            m.fall_start_y = y;
            m.vy = 0.0;
        }
        let mut hit = None;
        for _ in 0..80 {
            hit = step_player_motion(&mut world, 1, &PlayerIntent::default());
            if world.get::<Motion>(1).unwrap().on_ground {
                break;
            }
        }
        assert!(world.get::<Motion>(1).unwrap().on_ground);
        assert!(hit.map(|e| e.fall_damage > 0.0).unwrap_or(false));
    }

    #[test]
    fn stun_blocks_horizontal_wish() {
        let mut world = player_at(0.0, 0.0);
        let start_z = world.get::<Transform>(1).unwrap().z;
        world.insert(
            1,
            crate::ecs::components::Auras {
                auras: vec![crate::ecs::components::AuraInstance {
                    id: "cheap_shot".into(),
                    remaining: 2.0,
                    stacks: 1,
                    tick_timer: 99.0,
                    tick_interval: 0.0,
                    tick_damage: 0.0,
                    tick_heal: 0.0,
                    source: 2,
                    stun: true,
                    move_mult: 0.0,
                    absorb: 0.0,
                    breaks_on_damage: false,
                    damage_mult: 1.0,
                }],
            },
        );
        step_axes(&mut world, 1, 0.0, 1.0, 0.0);
        assert!(
            (world.get::<Transform>(1).unwrap().z - start_z).abs() < 1e-4,
            "stunned player must not walk"
        );
    }
}
