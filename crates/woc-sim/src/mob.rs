//! Wolf aggro, chase, leash, respawn, and social pack aggro.

use crate::ecs::components::{Auras, Combat, Health, Home, LootTable, Respawn, Threat, Transform};
use crate::ecs::World;
use crate::entity_motion::{step_toward, step_toward_home};
use crate::types::{AGGRO_RANGE, LEASH_RANGE, MELEE_RANGE, MOB_SPEED};
use crate::world::{ground_height, WORLD_SEED};
use woc_protocol::EntityId;

/// Seconds before a dead mob revives at home with full HP.
pub const MOB_RESPAWN_SEC: f32 = 30.0;
/// Distance from an engaged mob within which camp allies share aggro.
pub const SOCIAL_AGGRO_RANGE: f32 = 16.0;
/// Max home-to-home distance for two mobs to count as the same camp.
pub const CAMP_HOME_RADIUS: f32 = 20.0;

fn is_living_mob(world: &World, id: EntityId) -> bool {
    world.get::<LootTable>(id).is_some()
        && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
}

/// Count down dead-mob respawn timers; revive at home with full HP when ready.
///
/// Call once per sim tick (e.g. after combat / alongside aura ticks):
/// `tick_mob_respawns(world, DT)`.
pub fn tick_mob_respawns(world: &mut World, dt: f32) {
    let ids = world.ids::<Respawn>();
    for id in ids {
        if world.get::<Health>(id).map(|h| h.alive).unwrap_or(true) {
            continue;
        }
        let timer = world
            .get::<Respawn>(id)
            .map(|r| r.respawn_timer)
            .unwrap_or(0.0);
        if timer <= 0.0 {
            // First observation of death: arm the timer (full duration).
            if let Some(r) = world.get_mut::<Respawn>(id) {
                r.respawn_timer = MOB_RESPAWN_SEC;
            }
            continue;
        }
        let remaining = timer - dt;
        if remaining <= 0.0 {
            revive_mob(world, id);
        } else if let Some(r) = world.get_mut::<Respawn>(id) {
            r.respawn_timer = remaining;
        }
    }
}

fn revive_mob(world: &mut World, id: EntityId) {
    let hp_max = world.get::<Health>(id).map(|h| h.hp_max).unwrap_or(1.0);
    let home = world.get::<Home>(id).copied();
    if let Some(h) = world.get_mut::<Health>(id) {
        h.alive = true;
        h.hp = hp_max;
    }
    if let Some(home) = home {
        if let Some(t) = world.get_mut::<Transform>(id) {
            t.x = home.home_x;
            t.z = home.home_z;
            t.y = ground_height(t.x, t.z, WORLD_SEED);
        }
    }
    if let Some(c) = world.get_mut::<Combat>(id) {
        c.target = None;
        c.cast = None;
        c.swing_timer = 0.0;
        c.ability_cd = 0.0;
        c.gcd = 0.0;
    }
    if let Some(th) = world.get_mut::<Threat>(id) {
        th.threat.clear();
    }
    if let Some(a) = world.get_mut::<Auras>(id) {
        a.auras.clear();
    }
    if let Some(r) = world.get_mut::<Respawn>(id) {
        r.respawn_timer = 0.0;
    }
}

/// When `source` is engaged on `target`, nearby same-camp allies acquire that target.
pub fn apply_social_aggro(world: &mut World, source_id: EntityId, target: EntityId) {
    if !is_living_mob(world, source_id) {
        return;
    }
    let Some(st) = world.get::<Transform>(source_id).copied() else {
        return;
    };
    let Some(sh) = world.get::<Home>(source_id).copied() else {
        return;
    };

    let ally_ids: Vec<EntityId> = world
        .ids::<Home>()
        .into_iter()
        .filter(|&id| {
            id != source_id
                && is_living_mob(world, id)
                && world
                    .get::<Combat>(id)
                    .map(|c| c.target.is_none())
                    .unwrap_or(false)
                && world
                    .get::<Home>(id)
                    .map(|h| same_camp(sh.home_x, sh.home_z, h.home_x, h.home_z))
                    .unwrap_or(false)
                && world
                    .get::<Transform>(id)
                    .map(|t| dist_xz(st.x, st.z, t.x, t.z) <= SOCIAL_AGGRO_RANGE)
                    .unwrap_or(false)
        })
        .collect();

    for aid in ally_ids {
        if let Some(c) = world.get_mut::<Combat>(aid) {
            c.target = Some(target);
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

pub fn update_mob_ai(world: &mut World, mob_id: EntityId, player_id: EntityId) {
    if !is_living_mob(world, mob_id) {
        return;
    }
    let player_alive = world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false);
    if !player_alive {
        // Lost target / not in combat → return home.
        if let Some(c) = world.get_mut::<Combat>(mob_id) {
            c.target = None;
        }
        if let Some(t) = world.get_mut::<Threat>(mob_id) {
            t.threat.clear();
        }
        move_toward_home(world, mob_id);
        return;
    }

    let d_player = crate::ecs::components::dist2d(world, mob_id, player_id).unwrap_or(f32::MAX);
    let Some(t) = world.get::<Transform>(mob_id).copied() else {
        return;
    };
    let Some(home) = world.get::<Home>(mob_id).copied() else {
        return;
    };
    let d_home = dist_xz(t.x, t.z, home.home_x, home.home_z);

    // Leash: too far from home → drop combat and return.
    if d_home > LEASH_RANGE {
        if let Some(c) = world.get_mut::<Combat>(mob_id) {
            c.target = None;
        }
        if let Some(th) = world.get_mut::<Threat>(mob_id) {
            th.threat.clear();
        }
        move_toward_home(world, mob_id);
        return;
    }

    let mut just_engaged = false;
    let current_target = world.get::<Combat>(mob_id).and_then(|c| c.target);
    if current_target.is_none() && d_player <= AGGRO_RANGE {
        if let Some(c) = world.get_mut::<Combat>(mob_id) {
            c.target = Some(player_id);
        }
        just_engaged = true;
    }

    let engaged =
        just_engaged || world.get::<Combat>(mob_id).and_then(|c| c.target) == Some(player_id);
    if engaged {
        apply_social_aggro(world, mob_id, player_id);
    }

    let target = world.get::<Combat>(mob_id).and_then(|c| c.target);
    if target == Some(player_id) {
        if d_player > MELEE_RANGE * 0.85 {
            let Some(pt) = world.get::<Transform>(player_id).copied() else {
                return;
            };
            let speed = MOB_SPEED * crate::combat::move_speed_mult(world, mob_id);
            if crate::combat::is_stunned(world, mob_id) {
                return;
            }
            let _ = step_toward(world, mob_id, pt.x, pt.z, speed);
        }
    } else {
        // Not in combat → return home.
        move_toward_home(world, mob_id);
    }
}

fn move_toward_home(world: &mut World, mob_id: EntityId) {
    if step_toward_home(world, mob_id, MOB_SPEED * 0.85, 0.2) {
        // Reset after leash / idle return.
        if let Some(h) = world.get_mut::<Health>(mob_id) {
            h.hp = h.hp_max;
        }
        if let Some(t) = world.get_mut::<Threat>(mob_id) {
            t.threat.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Combat, Health, Home, Respawn, Threat, Transform};
    use crate::types::LEASH_RANGE;
    use woc_content::PlayerClass;
    use woc_protocol::DT;

    #[test]
    fn dead_mob_respawns_after_timer() {
        let mut world = World::new();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(2) {
            h.alive = false;
            h.hp = 0.0;
        }
        // First tick arms the full respawn timer.
        tick_mob_respawns(&mut world, DT);
        assert!(!world.get::<Health>(2).unwrap().alive);
        assert!(world.get::<Respawn>(2).unwrap().respawn_timer > MOB_RESPAWN_SEC - 1.0);
        // Force expiry.
        if let Some(r) = world.get_mut::<Respawn>(2) {
            r.respawn_timer = DT;
        }
        tick_mob_respawns(&mut world, DT);
        assert!(world.get::<Health>(2).unwrap().alive);
        assert!(world.get::<Health>(2).unwrap().hp > 0.0);
    }

    #[test]
    fn mob_chases_player_in_aggro() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 5.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        let before = world.get::<Transform>(2).unwrap().x;
        update_mob_ai(&mut world, 2, 1);
        assert!(world.get::<Transform>(2).unwrap().x > before);
        assert_eq!(world.get::<Combat>(2).unwrap().target, Some(1));
    }

    #[test]
    fn leash_clears_target_and_returns_home_when_too_far() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = LEASH_RANGE + 5.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.target = Some(1);
        }
        if let Some(th) = world.get_mut::<Threat>(2) {
            th.threat.insert(1, 10.0);
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = h.hp_max * 0.4;
        }
        update_mob_ai(&mut world, 2, 1);
        assert!(
            world.get::<Combat>(2).unwrap().target.is_none(),
            "leash drops target"
        );
        assert!(
            world.get::<Threat>(2).unwrap().threat.is_empty(),
            "leash clears threat"
        );
        assert!(world.get::<Transform>(2).unwrap().x < LEASH_RANGE + 5.0 - 0.01);
    }

    #[test]
    fn lost_target_returns_home_when_player_dead() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 5.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.alive = false;
            h.hp = 0.0;
        }
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 4.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.target = Some(1);
        }
        update_mob_ai(&mut world, 2, 1);
        assert!(world.get::<Combat>(2).unwrap().target.is_none());
        assert!(
            world.get::<Transform>(2).unwrap().x < 4.0 - 0.01,
            "should walk home after losing target"
        );
    }

    #[test]
    fn social_aggro_pulls_nearby_same_camp_ally() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 4.0, 0.0).unwrap();
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 2.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(t) = world.get_mut::<Transform>(3) {
            t.x = 5.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        update_mob_ai(&mut world, 2, 1);
        assert_eq!(
            world.get::<Combat>(2).unwrap().target,
            Some(1),
            "engager acquires player"
        );
        assert_eq!(
            world.get::<Combat>(3).unwrap().target,
            Some(1),
            "camp ally social-aggros same target"
        );
    }

    #[test]
    fn social_aggro_ignores_distant_camp() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 50.0, 50.0)
            .unwrap();
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 2.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(t) = world.get_mut::<Transform>(3) {
            t.x = 3.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        update_mob_ai(&mut world, 2, 1);
        assert!(
            world.get::<Combat>(3).unwrap().target.is_none(),
            "different camp must not social-aggro"
        );
    }

    #[test]
    fn arrive_home_after_leash_restores_full_hp() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 100.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 0.1;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 10.0;
        }
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.target = None;
        }
        update_mob_ai(&mut world, 2, 1);
        let home = world.get::<Home>(2).unwrap();
        let t = world.get::<Transform>(2).unwrap();
        assert!((t.x - home.home_x).abs() < 1e-3);
        let h = world.get::<Health>(2).unwrap();
        assert!((h.hp - h.hp_max).abs() < 1e-3);
    }
}
