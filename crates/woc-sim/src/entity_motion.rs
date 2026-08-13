//! Shared ground locomotion for non-player actors (mobs, pets, NPC wander).

use crate::ecs::components::{Home, Transform};
use crate::ecs::World;
use crate::player_motion::ground_step;
use crate::world::{clamp_to_world, ground_height, WORLD_SEED};
use woc_protocol::{EntityId, DT};

/// Move pose toward `(tx, tz)` at `speed` yards/sec, clamping to walkable ground.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_toward_advances_and_faces_target() {
        let mut world = World::new();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0)
            .expect("wolf");
        let before = world.get::<Transform>(2).unwrap().x;
        let traveled = step_toward(&mut world, 2, 10.0, 0.0, 5.0);
        let t = world.get::<Transform>(2).unwrap();
        assert!(traveled > 0.01);
        assert!(t.x > before);
        assert!((t.y - ground_height(t.x, t.z, WORLD_SEED)).abs() < 1e-3);
    }

    #[test]
    fn step_toward_home_snaps_when_close() {
        let mut world = World::new();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0)
            .expect("wolf");
        if let Some(h) = world.get_mut::<Home>(2) {
            h.home_x = 0.0;
            h.home_z = 0.0;
        }
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 0.2;
            t.z = 0.0;
        }
        assert!(step_toward_home(&mut world, 2, 5.0, 0.5));
        let t = world.get::<Transform>(2).unwrap();
        assert!((t.x - 0.0).abs() < 1e-5);
        assert!((t.z - 0.0).abs() < 1e-5);
    }
}
