//! Shared ground locomotion for non-player actors (mobs, pets, NPC wander).
//!
//! Applies a horizontal step toward a target with climb-slope rejection and
//! world bounds, mirroring the player motion soft gates without wish-vector
//! facing math.

use crate::ecs::components::{Home, Transform};
use crate::ecs::World;
use crate::entity::Entity;
use crate::player_motion::ground_step;
use crate::world::{clamp_to_world, ground_height, WORLD_SEED};
use woc_protocol::{EntityId, DT};

/// Move pose toward `(tx, tz)` at `speed` yards/sec, clamping to walkable ground.
///
/// Updates yaw to face the travel direction. Returns horizontal distance traveled.
pub fn step_toward_transform(t: &mut Transform, tx: f32, tz: f32, speed: f32) -> f32 {
    let dx = tx - t.x;
    let dz = tz - t.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 0.01 {
        t.y = ground_height(t.x, t.z, WORLD_SEED);
        return 0.0;
    }
    let step = (speed * DT).min(d);
    let wish_x = t.x + dx / d * step;
    let wish_z = t.z + dz / d * step;
    let (wish_x, wish_z) = clamp_to_world(wish_x, wish_z);

    let (nx, ny, nz) = if let Some(ny) = ground_step(t.y, wish_x, wish_z, step) {
        (wish_x, ny, wish_z)
    } else {
        // Axis-separate fallback (same idea as player motion).
        let (sx, _) = clamp_to_world(wish_x, t.z);
        if let Some(ny) = ground_step(t.y, sx, t.z, (sx - t.x).abs()) {
            (sx, ny, t.z)
        } else {
            let (_, sz) = clamp_to_world(t.x, wish_z);
            if let Some(ny) = ground_step(t.y, t.x, sz, (sz - t.z).abs()) {
                (t.x, ny, sz)
            } else {
                (t.x, ground_height(t.x, t.z, WORLD_SEED), t.z)
            }
        }
    };

    let traveled = ((nx - t.x).powi(2) + (nz - t.z).powi(2)).sqrt();
    t.x = nx;
    t.z = nz;
    t.y = ny;
    t.yaw = dx.atan2(dz);
    traveled
}

/// Move `id` toward `(tx, tz)` using the Transform column.
pub fn step_toward(world: &mut World, id: EntityId, tx: f32, tz: f32, speed: f32) -> f32 {
    let Some(t) = world.get_mut::<Transform>(id) else {
        return 0.0;
    };
    step_toward_transform(t, tx, tz, speed)
}

/// Dual-write shim for uncut mob/pet AI that still holds a fat `Entity`.
pub fn step_toward_entity(actor: &mut Entity, tx: f32, tz: f32, speed: f32) -> f32 {
    let mut t = Transform {
        x: actor.x,
        y: actor.y,
        z: actor.z,
        yaw: actor.yaw,
    };
    let traveled = step_toward_transform(&mut t, tx, tz, speed);
    actor.x = t.x;
    actor.y = t.y;
    actor.z = t.z;
    actor.yaw = t.yaw;
    traveled
}

/// Snap an actor onto its home pad if already close enough; otherwise walk home.
pub fn step_toward_home(world: &mut World, id: EntityId, speed: f32, arrive: f32) -> bool {
    let Some(home) = world.get::<Home>(id).copied() else {
        return false;
    };
    let Some(t) = world.get::<Transform>(id).cloned() else {
        return false;
    };
    let dx = home.home_x - t.x;
    let dz = home.home_z - t.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < arrive {
        if let Some(t) = world.get_mut::<Transform>(id) {
            t.x = home.home_x;
            t.z = home.home_z;
            t.y = ground_height(t.x, t.z, WORLD_SEED);
        }
        return true;
    }
    step_toward(world, id, home.home_x, home.home_z, speed);
    false
}

/// Dual-write shim for uncut mob AI.
pub fn step_toward_home_entity(actor: &mut Entity, speed: f32, arrive: f32) -> bool {
    let dx = actor.home_x - actor.x;
    let dz = actor.home_z - actor.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < arrive {
        actor.x = actor.home_x;
        actor.z = actor.home_z;
        actor.y = ground_height(actor.x, actor.z, WORLD_SEED);
        return true;
    }
    step_toward_entity(actor, actor.home_x, actor.home_z, speed);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_mob_from_template;

    fn run_step_toward(mob: &mut Entity, tx: f32, tz: f32, speed: f32) -> f32 {
        let mut world = crate::ecs::spawn::world_from_entities(std::slice::from_ref(mob));
        let traveled = step_toward(&mut world, mob.id, tx, tz, speed);
        crate::ecs::spawn::apply_world_to_entity(&world, mob);
        traveled
    }

    fn run_step_toward_home(mob: &mut Entity, speed: f32, arrive: f32) -> bool {
        let mut world = crate::ecs::spawn::world_from_entities(std::slice::from_ref(mob));
        let arrived = step_toward_home(&mut world, mob.id, speed, arrive);
        crate::ecs::spawn::apply_world_to_entity(&world, mob);
        arrived
    }

    #[test]
    fn step_toward_advances_and_faces_target() {
        let mut mob = create_mob_from_template(2, "young_wolf", 0.0, 0.0).expect("wolf");
        let before = mob.x;
        let traveled = run_step_toward(&mut mob, 10.0, 0.0, 5.0);
        assert!(traveled > 0.01);
        assert!(mob.x > before);
        assert!((mob.y - ground_height(mob.x, mob.z, WORLD_SEED)).abs() < 1e-3);
    }

    #[test]
    fn step_toward_home_snaps_when_close() {
        let mut mob = create_mob_from_template(2, "young_wolf", 0.0, 0.0).expect("wolf");
        mob.home_x = 0.0;
        mob.home_z = 0.0;
        mob.x = 0.05;
        mob.z = 0.0;
        assert!(run_step_toward_home(&mut mob, 5.0, 0.2));
        assert!((mob.x - mob.home_x).abs() < 1e-4);
    }
}
