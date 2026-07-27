//! Wolf aggro, chase, and leash.

use crate::combat::dist2d;
use crate::entity::Entity;
use crate::types::{AGGRO_RANGE, LEASH_RANGE, MELEE_RANGE, WOLF_SPEED};
use crate::world::terrain_height;
use crate::world::WORLD_SEED;
use woc_protocol::{EntityId, EntityKind, DT};

pub fn update_mob_ai(mob_id: EntityId, player_id: EntityId, entities: &mut [Entity]) {
    let Some(mi) = entities.iter().position(|e| e.id == mob_id) else {
        return;
    };
    if !entities[mi].alive || entities[mi].kind != EntityKind::Mob {
        return;
    }
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    if !entities[pi].alive {
        // Return home.
        move_toward_home(&mut entities[mi]);
        return;
    }

    let d_player = dist2d(&entities[mi], &entities[pi]);
    let home_dx = entities[mi].x - entities[mi].home_x;
    let home_dz = entities[mi].z - entities[mi].home_z;
    let d_home = (home_dx * home_dx + home_dz * home_dz).sqrt();

    if d_home > LEASH_RANGE {
        entities[mi].target = None;
        move_toward_home(&mut entities[mi]);
        return;
    }

    if entities[mi].target.is_none() && d_player <= AGGRO_RANGE {
        entities[mi].target = Some(player_id);
    }

    let (px, pz) = (entities[pi].x, entities[pi].z);
    if entities[mi].target == Some(player_id) {
        if d_player > MELEE_RANGE * 0.85 {
            move_toward(&mut entities[mi], px, pz, WOLF_SPEED);
        }
    } else {
        move_toward_home(&mut entities[mi]);
    }
}

fn move_toward(mob: &mut Entity, tx: f32, tz: f32, speed: f32) {
    let dx = tx - mob.x;
    let dz = tz - mob.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 0.01 {
        return;
    }
    let step = speed * DT;
    let nx = mob.x + dx / d * step.min(d);
    let nz = mob.z + dz / d * step.min(d);
    mob.x = nx;
    mob.z = nz;
    mob.y = terrain_height(mob.x, mob.z, WORLD_SEED);
    mob.yaw = dx.atan2(dz);
}

fn move_toward_home(mob: &mut Entity) {
    let dx = mob.home_x - mob.x;
    let dz = mob.home_z - mob.z;
    let d = (dx * dx + dz * dz).sqrt();
    if d < 0.2 {
        mob.x = mob.home_x;
        mob.z = mob.home_z;
        mob.y = terrain_height(mob.x, mob.z, WORLD_SEED);
        return;
    }
    move_toward(mob, mob.home_x, mob.home_z, WOLF_SPEED * 0.85);
}
