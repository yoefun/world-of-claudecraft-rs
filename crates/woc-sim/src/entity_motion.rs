//! Shared ground locomotion for non-player actors (mobs, pets, NPC wander).
//!
//! Applies a horizontal step toward a target with climb-slope rejection and
//! world bounds, mirroring the player motion soft gates without wish-vector
//! facing math.

use crate::entity::Entity;
use crate::player_motion::ground_step;
use crate::world::{clamp_to_world, ground_height, WORLD_SEED};
use woc_protocol::DT;

/// Move `actor` toward `(tx, tz)` at `speed` yards/sec, clamping to walkable ground.
///
/// Updates yaw to face the travel direction. Returns horizontal distance traveled.
pub fn step_toward(actor: &mut Entity, tx: f32, tz: f32, speed: f32) -> f32 {
    let dx = tx - actor.x;
    let dz = tz - actor.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 0.01 {
        actor.y = ground_height(actor.x, actor.z, WORLD_SEED);
        return 0.0;
    }
    let step = (speed * DT).min(d);
    let wish_x = actor.x + dx / d * step;
    let wish_z = actor.z + dz / d * step;
    let (wish_x, wish_z) = clamp_to_world(wish_x, wish_z);

    let (nx, ny, nz) = if let Some(ny) = ground_step(actor.y, wish_x, wish_z, step) {
        (wish_x, ny, wish_z)
    } else {
        // Axis-separate fallback (same idea as player motion).
        let (sx, _) = clamp_to_world(wish_x, actor.z);
        if let Some(ny) = ground_step(actor.y, sx, actor.z, (sx - actor.x).abs()) {
            (sx, ny, actor.z)
        } else {
            let (_, sz) = clamp_to_world(actor.x, wish_z);
            if let Some(ny) = ground_step(actor.y, actor.x, sz, (sz - actor.z).abs()) {
                (actor.x, ny, sz)
            } else {
                (
                    actor.x,
                    ground_height(actor.x, actor.z, WORLD_SEED),
                    actor.z,
                )
            }
        }
    };

    let traveled = ((nx - actor.x).powi(2) + (nz - actor.z).powi(2)).sqrt();
    actor.x = nx;
    actor.z = nz;
    actor.y = ny;
    actor.yaw = dx.atan2(dz);
    traveled
}

/// Snap an actor onto its home pad if already close enough; otherwise walk home.
pub fn step_toward_home(actor: &mut Entity, speed: f32, arrive: f32) -> bool {
    let dx = actor.home_x - actor.x;
    let dz = actor.home_z - actor.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < arrive {
        actor.x = actor.home_x;
        actor.z = actor.home_z;
        actor.y = ground_height(actor.x, actor.z, WORLD_SEED);
        return true;
    }
    step_toward(actor, actor.home_x, actor.home_z, speed);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_mob_from_template;

    #[test]
    fn step_toward_advances_and_faces_target() {
        let mut mob = create_mob_from_template(2, "young_wolf", 0.0, 0.0).expect("wolf");
        let before = mob.x;
        let traveled = step_toward(&mut mob, 10.0, 0.0, 5.0);
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
        assert!(step_toward_home(&mut mob, 5.0, 0.2));
        assert!((mob.x - mob.home_x).abs() < 1e-4);
    }
}
