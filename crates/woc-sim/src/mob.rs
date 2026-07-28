//! Wolf aggro, chase, leash, respawn, and social pack aggro.

use crate::combat::dist2d;
use crate::entity::Entity;
use crate::types::{AGGRO_RANGE, LEASH_RANGE, MELEE_RANGE, MOB_SPEED};
use crate::world::terrain_height;
use crate::world::WORLD_SEED;
use woc_protocol::{EntityId, EntityKind, DT};

/// Seconds before a dead mob revives at home with full HP.
pub const MOB_RESPAWN_SEC: f32 = 30.0;
/// Distance from an engaged mob within which camp allies share aggro.
pub const SOCIAL_AGGRO_RANGE: f32 = 16.0;
/// Max home-to-home distance for two mobs to count as the same camp.
pub const CAMP_HOME_RADIUS: f32 = 20.0;

/// Count down dead-mob respawn timers; revive at home with full HP when ready.
///
/// Call once per sim tick (e.g. after combat / alongside aura ticks):
/// `tick_mob_respawns(&mut entities, DT)`.
pub fn tick_mob_respawns(entities: &mut [Entity], dt: f32) {
    for e in entities.iter_mut() {
        if e.kind != EntityKind::Mob || e.alive {
            continue;
        }
        if e.respawn_timer <= 0.0 {
            // First observation of death: arm the timer (full duration).
            e.respawn_timer = MOB_RESPAWN_SEC;
            continue;
        }
        e.respawn_timer -= dt;
        if e.respawn_timer <= 0.0 {
            revive_mob(e);
        }
    }
}

fn revive_mob(mob: &mut Entity) {
    mob.alive = true;
    mob.hp = mob.hp_max;
    mob.x = mob.home_x;
    mob.z = mob.home_z;
    mob.y = terrain_height(mob.x, mob.z, WORLD_SEED);
    mob.target = None;
    mob.threat.clear();
    mob.auras.clear();
    mob.cast = None;
    mob.swing_timer = 0.0;
    mob.ability_cd = 0.0;
    mob.gcd = 0.0;
    mob.respawn_timer = 0.0;
}

/// When `source` is engaged on `target`, nearby same-camp allies acquire that target.
pub fn apply_social_aggro(source_id: EntityId, target: EntityId, entities: &mut [Entity]) {
    let Some(si) = entities.iter().position(|e| e.id == source_id) else {
        return;
    };
    if !entities[si].alive || entities[si].kind != EntityKind::Mob {
        return;
    }
    let (sx, sz) = (entities[si].x, entities[si].z);
    let (shx, shz) = (entities[si].home_x, entities[si].home_z);

    let ally_ids: Vec<EntityId> = entities
        .iter()
        .filter(|e| {
            e.id != source_id
                && e.kind == EntityKind::Mob
                && e.alive
                && e.target.is_none()
                && same_camp(shx, shz, e.home_x, e.home_z)
                && dist_xz(sx, sz, e.x, e.z) <= SOCIAL_AGGRO_RANGE
        })
        .map(|e| e.id)
        .collect();

    for aid in ally_ids {
        if let Some(e) = entities.iter_mut().find(|e| e.id == aid) {
            e.target = Some(target);
        }
    }
}

fn same_camp(ax: f32, az: f32, bx: f32, bz: f32) -> bool {
    dist_xz(ax, az, bx, bz) <= CAMP_HOME_RADIUS
}

fn dist_xz(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

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
        // Lost target / not in combat → return home.
        entities[mi].target = None;
        entities[mi].threat.clear();
        move_toward_home(&mut entities[mi]);
        return;
    }

    let d_player = dist2d(&entities[mi], &entities[pi]);
    let home_dx = entities[mi].x - entities[mi].home_x;
    let home_dz = entities[mi].z - entities[mi].home_z;
    let d_home = (home_dx * home_dx + home_dz * home_dz).sqrt();

    // Leash: too far from home → drop combat and return.
    if d_home > LEASH_RANGE {
        entities[mi].target = None;
        entities[mi].threat.clear();
        move_toward_home(&mut entities[mi]);
        return;
    }

    let mut just_engaged = false;
    if entities[mi].target.is_none() && d_player <= AGGRO_RANGE {
        entities[mi].target = Some(player_id);
        just_engaged = true;
    }

    if just_engaged || entities[mi].target == Some(player_id) {
        apply_social_aggro(mob_id, player_id, entities);
    }

    // Re-resolve index after social aggro (borrow ended).
    let Some(mi) = entities.iter().position(|e| e.id == mob_id) else {
        return;
    };
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };

    let (px, pz) = (entities[pi].x, entities[pi].z);
    if entities[mi].target == Some(player_id) {
        if d_player > MELEE_RANGE * 0.85 {
            move_toward(&mut entities[mi], px, pz, MOB_SPEED);
        }
    } else {
        // Not in combat → return home.
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
        // Reset after leash / idle return.
        mob.hp = mob.hp_max;
        mob.threat.clear();
        return;
    }
    move_toward(mob, mob.home_x, mob.home_z, MOB_SPEED * 0.85);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player};
    use crate::types::LEASH_RANGE;
    use woc_content::PlayerClass;

    fn wolf(id: EntityId, x: f32, z: f32) -> Entity {
        let mut m = create_mob_from_template(id, "young_wolf", x, z).expect("wolf");
        m.home_x = x;
        m.home_z = z;
        m
    }

    #[test]
    fn dead_mob_arms_respawn_timer_then_revives_at_home_full_hp() {
        let mut mob = wolf(2, 10.0, -5.0);
        mob.x = 3.0;
        mob.z = 1.0;
        mob.hp = 0.0;
        mob.alive = false;
        mob.target = Some(1);
        mob.threat.insert(1, 50.0);

        let mut entities = vec![mob];

        tick_mob_respawns(&mut entities, DT);
        assert!(
            (entities[0].respawn_timer - MOB_RESPAWN_SEC).abs() < 1e-4,
            "first tick arms full respawn timer"
        );
        assert!(!entities[0].alive);

        // Almost ready: leave a sliver of timer.
        entities[0].respawn_timer = DT * 0.5;
        tick_mob_respawns(&mut entities, DT);

        assert!(entities[0].alive, "mob should revive when timer elapses");
        assert!((entities[0].hp - entities[0].hp_max).abs() < 1e-3);
        assert!((entities[0].x - entities[0].home_x).abs() < 1e-3);
        assert!((entities[0].z - entities[0].home_z).abs() < 1e-3);
        assert!(entities[0].target.is_none());
        assert!(entities[0].threat.is_empty());
        assert_eq!(entities[0].respawn_timer, 0.0);
    }

    #[test]
    fn leash_clears_target_and_returns_home_when_too_far() {
        let player = create_player(1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        let mut mob = wolf(2, 0.0, 0.0);
        // Past leash range from home, still "engaged".
        mob.x = LEASH_RANGE + 5.0;
        mob.z = 0.0;
        mob.y = Entity::ground_at(mob.x, mob.z);
        mob.target = Some(player.id);
        mob.threat.insert(player.id, 10.0);
        mob.hp = mob.hp_max * 0.4;

        let mut entities = vec![player, mob];
        update_mob_ai(2, 1, &mut entities);

        let mob = entities.iter().find(|e| e.id == 2).unwrap();
        assert!(mob.target.is_none(), "leash drops target");
        assert!(mob.threat.is_empty(), "leash clears threat");
        // Moved toward home (x should decrease from LEASH+5 toward 0).
        assert!(mob.x < LEASH_RANGE + 5.0 - 0.01);
    }

    #[test]
    fn lost_target_returns_home_when_player_dead() {
        let mut player = create_player(1, "Hero", PlayerClass::Warrior, 5.0, 0.0);
        player.alive = false;
        player.hp = 0.0;
        let mut mob = wolf(2, 0.0, 0.0);
        mob.x = 4.0;
        mob.z = 0.0;
        mob.y = Entity::ground_at(mob.x, mob.z);
        mob.target = Some(1);

        let mut entities = vec![player, mob];
        update_mob_ai(2, 1, &mut entities);

        let mob = entities.iter().find(|e| e.id == 2).unwrap();
        assert!(mob.target.is_none());
        assert!(mob.x < 4.0 - 0.01, "should walk home after losing target");
    }

    #[test]
    fn social_aggro_pulls_nearby_same_camp_ally() {
        let player = create_player(1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        // Camp homes within CAMP_HOME_RADIUS; ally within SOCIAL_AGGRO_RANGE of engager.
        let mut a = wolf(2, 0.0, 0.0);
        let mut b = wolf(3, 4.0, 0.0);
        // Place both near player so A can aggro; B has no target yet.
        a.x = 2.0;
        a.z = 0.0;
        a.y = Entity::ground_at(a.x, a.z);
        b.x = 5.0;
        b.z = 0.0;
        b.y = Entity::ground_at(b.x, b.z);

        let mut entities = vec![player, a, b];
        update_mob_ai(2, 1, &mut entities);

        let a = entities.iter().find(|e| e.id == 2).unwrap();
        let b = entities.iter().find(|e| e.id == 3).unwrap();
        assert_eq!(a.target, Some(1), "engager acquires player");
        assert_eq!(b.target, Some(1), "camp ally social-aggros same target");
    }

    #[test]
    fn social_aggro_ignores_distant_camp() {
        let player = create_player(1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        let mut a = wolf(2, 0.0, 0.0);
        // Far home → different camp.
        let mut b = wolf(3, 50.0, 50.0);
        a.x = 2.0;
        a.z = 0.0;
        a.y = Entity::ground_at(a.x, a.z);
        // Physically near A but not same camp by home.
        b.x = 3.0;
        b.z = 0.0;
        b.y = Entity::ground_at(b.x, b.z);

        let mut entities = vec![player, a, b];
        update_mob_ai(2, 1, &mut entities);

        let b = entities.iter().find(|e| e.id == 3).unwrap();
        assert!(b.target.is_none(), "different camp must not social-aggro");
    }

    #[test]
    fn arrive_home_after_leash_restores_full_hp() {
        let player = create_player(1, "Hero", PlayerClass::Warrior, 100.0, 0.0);
        let mut mob = wolf(2, 0.0, 0.0);
        mob.x = 0.1;
        mob.z = 0.0;
        mob.y = Entity::ground_at(mob.x, mob.z);
        mob.hp = 10.0;
        mob.target = None;

        let mut entities = vec![player, mob];
        update_mob_ai(2, 1, &mut entities);

        let mob = entities.iter().find(|e| e.id == 2).unwrap();
        assert!((mob.x - mob.home_x).abs() < 1e-3);
        assert!((mob.hp - mob.hp_max).abs() < 1e-3);
    }
}
