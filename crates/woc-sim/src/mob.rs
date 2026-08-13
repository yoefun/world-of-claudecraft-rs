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
        let timer = world.get::<Respawn>(id).map(|r| r.respawn_timer).unwrap_or(0.0);
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

    let engaged = just_engaged
        || world.get::<Combat>(mob_id).and_then(|c| c.target) == Some(player_id);
    if engaged {
        apply_social_aggro(world, mob_id, player_id);
    }

    let target = world.get::<Combat>(mob_id).and_then(|c| c.target);
    if target == Some(player_id) {
        if d_player > MELEE_RANGE * 0.85 {
            let Some(pt) = world.get::<Transform>(player_id).copied() else {
                return;
            };
            let _ = step_toward(world, mob_id, pt.x, pt.z, MOB_SPEED);
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
    use crate::entity::{create_mob_from_template, create_player, Entity};
    use crate::types::LEASH_RANGE;
    use woc_content::PlayerClass;
    use woc_protocol::DT;

    fn wolf(id: EntityId, x: f32, z: f32) -> Entity {
        let mut m = create_mob_from_template(id, "young_wolf", x, z).expect("wolf");
        m.home_x = x;
        m.home_z = z;
        m
    }

    fn run_respawns(entities: &mut [Entity], dt: f32) {
        let mut world = crate::ecs::spawn::world_from_entities(entities);
        tick_mob_respawns(&mut world, dt);
        crate::ecs::spawn::apply_world_to_entities(&world, entities);
    }

    fn run_ai(entities: &mut [Entity], mob_id: EntityId, player_id: EntityId) {
        let mut world = crate::ecs::spawn::world_from_entities(entities);
        update_mob_ai(&mut world, mob_id, player_id);
        crate::ecs::spawn::apply_world_to_entities(&world, entities);
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

        run_respawns(&mut entities, DT);
        assert!(
            (entities[0].respawn_timer - MOB_RESPAWN_SEC).abs() < 1e-4,
            "first tick arms full respawn timer"
        );
        assert!(!entities[0].alive);

        // Almost ready: leave a sliver of timer.
        entities[0].respawn_timer = DT * 0.5;
        run_respawns(&mut entities, DT);

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
        run_ai(&mut entities, 2, 1);

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
        run_ai(&mut entities, 2, 1);

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
        run_ai(&mut entities, 2, 1);

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
        run_ai(&mut entities, 2, 1);

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
        run_ai(&mut entities, 2, 1);

        let mob = entities.iter().find(|e| e.id == 2).unwrap();
        assert!((mob.x - mob.home_x).abs() < 1e-3);
        assert!((mob.hp - mob.hp_max).abs() < 1e-3);
    }
}
