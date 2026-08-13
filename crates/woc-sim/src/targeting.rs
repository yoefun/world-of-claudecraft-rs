//! Tab-target cycling among living hostile mobs.

use crate::ecs::components::{ClassKit, Combat, Health, LootTable, Owner, Transform};
use crate::ecs::World;
use woc_protocol::EntityId;

/// Max range for tab-target candidates (same ballpark as combat acquire).
pub const TAB_TARGET_RANGE: f32 = 40.0;

fn angle_delta(player_yaw: f32, from_x: f32, from_z: f32, to_x: f32, to_z: f32) -> f32 {
    let dx = to_x - from_x;
    let dz = to_z - from_z;
    let facing_to = dz.atan2(dx);
    let mut delta = facing_to - player_yaw;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta.abs()
}

/// Cycle candidates `(id, x, z)` by facing angle, then distance.
///
/// Shared by sim entities and client snapshot-based tab targeting.
pub fn tab_target_pose(
    player_x: f32,
    player_z: f32,
    player_yaw: f32,
    current: Option<EntityId>,
    candidates: &[(EntityId, f32, f32)],
) -> Option<EntityId> {
    let mut ranked: Vec<(EntityId, f32, f32)> = candidates
        .iter()
        .filter_map(|&(id, x, z)| {
            let dx = x - player_x;
            let dz = z - player_z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > TAB_TARGET_RANGE {
                return None;
            }
            Some((id, angle_delta(player_yaw, player_x, player_z, x, z), d))
        })
        .collect();

    if ranked.is_empty() {
        return None;
    }

    ranked.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });

    if let Some(cur) = current {
        if let Some(idx) = ranked.iter().position(|(id, ..)| *id == cur) {
            let next = (idx + 1) % ranked.len();
            return Some(ranked[next].0);
        }
    }
    Some(ranked[0].0)
}

/// Living hostile mobs: LootTable + Health.alive, not a pet or player.
fn is_living_hostile_mob(world: &World, id: EntityId) -> bool {
    world.get::<LootTable>(id).is_some()
        && world.get::<Combat>(id).is_some()
        && world.get::<Owner>(id).is_none()
        && world.get::<ClassKit>(id).is_none()
        && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
}

/// Cycle living hostile mobs by facing angle, then distance.
///
/// If the player already has a candidate targeted, returns the next in the
/// sorted cycle; otherwise returns the best (smallest angle, then nearest).
pub fn tab_target(world: &World, player_id: EntityId) -> Option<EntityId> {
    let health = world.get::<Health>(player_id)?;
    if !health.alive {
        return None;
    }
    let t = world.get::<Transform>(player_id)?;
    let current = world.get::<Combat>(player_id).and_then(|c| c.target);

    let candidates: Vec<(EntityId, f32, f32)> = world
        .ids::<LootTable>()
        .into_iter()
        .filter(|&id| is_living_hostile_mob(world, id))
        .filter_map(|id| {
            let tr = world.get::<Transform>(id)?;
            Some((id, tr.x, tr.z))
        })
        .collect();

    tab_target_pose(t.x, t.z, t.yaw, current, &candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player, Entity};
    use woc_content::PlayerClass;
    use woc_protocol::EntityKind;

    fn tab(entities: &[Entity], player_id: EntityId) -> Option<EntityId> {
        let world = crate::ecs::spawn::world_from_entities(entities);
        tab_target(&world, player_id)
    }

    #[test]
    fn tab_cycles_at_least_two_mobs() {
        let mut player = create_player(1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        player.yaw = 0.0;

        let mut a = create_mob_from_template(2, "young_wolf", 5.0, 0.0).unwrap();
        let mut b = create_mob_from_template(3, "young_wolf", 0.0, 5.0).unwrap();
        a.alive = true;
        b.alive = true;

        let entities = vec![player.clone(), a, b];
        let first = tab(&entities, 1).expect("first target");
        assert!(first == 2 || first == 3);

        let mut entities2 = entities;
        entities2[0].target = Some(first);
        let second = tab(&entities2, 1).expect("second target");
        assert_ne!(second, first);
        assert!(second == 2 || second == 3);

        entities2[0].target = Some(second);
        let third = tab(&entities2, 1).expect("cycles back");
        assert_eq!(third, first);
    }

    #[test]
    fn tab_ignores_dead_mobs() {
        let player = create_player(1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        let mut dead = create_mob_from_template(2, "young_wolf", 3.0, 0.0).unwrap();
        dead.alive = false;
        let live = create_mob_from_template(3, "young_wolf", 0.0, 3.0).unwrap();
        let entities = vec![player, dead, live];
        assert_eq!(tab(&entities, 1), Some(3));
    }

    #[test]
    fn sim_tab_target_and_clear() {
        use crate::sim::Sim;

        let mut sim = Sim::new_eastbrook("Tabber", PlayerClass::Warrior);
        let (px, pz) = {
            let p = sim.player().unwrap();
            (p.x, p.z)
        };
        let mut placed = 0u32;
        for e in sim.entities.iter_mut() {
            if e.kind == EntityKind::Mob && e.alive && placed < 2 {
                e.x = px + 2.0 + placed as f32;
                e.z = pz;
                e.y = crate::entity::Entity::ground_at(e.x, e.z);
                placed += 1;
            }
        }
        assert!(placed >= 2);

        let first = sim.tab_target().expect("tab finds a mob");
        assert_eq!(sim.player().unwrap().target, Some(first));

        let second = sim.tab_target().expect("tab cycles");
        assert_ne!(first, second);

        if let Some(p) = sim.player_mut() {
            p.auto_attack = true;
        }
        sim.clear_target();
        assert!(sim.player().unwrap().target.is_none());
        assert!(!sim.player().unwrap().auto_attack);
    }

    #[test]
    fn tab_cycles_eastbrook_wolves() {
        use crate::sim::Sim;

        let mut sim = Sim::new_eastbrook("Tabber", PlayerClass::Warrior);
        let (px, pz) = {
            let p = sim.player().unwrap();
            (p.x, p.z)
        };
        let mut wolf_ids = Vec::new();
        for e in sim.entities.iter_mut() {
            if e.kind == EntityKind::Mob
                && e.alive
                && e.template_id.as_deref() == Some("young_wolf")
                && wolf_ids.len() < 2
            {
                e.x = px + 2.0 + wolf_ids.len() as f32;
                e.z = pz;
                e.y = crate::entity::Entity::ground_at(e.x, e.z);
                wolf_ids.push(e.id);
            }
        }
        assert!(
            wolf_ids.len() >= 2,
            "eastbrook should spawn at least two young wolves"
        );

        let first = sim.tab_target().expect("tab finds a wolf");
        assert!(wolf_ids.contains(&first));
        let second = sim.tab_target().expect("tab cycles wolves");
        assert_ne!(first, second);
        assert!(wolf_ids.contains(&second));
    }

    #[test]
    fn tab_target_pose_matches_entity_tab() {
        let mut player = create_player(1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        player.yaw = 0.0;
        let a = create_mob_from_template(2, "young_wolf", 5.0, 0.0).unwrap();
        let b = create_mob_from_template(3, "young_wolf", 0.0, 5.0).unwrap();
        let entities = vec![player.clone(), a, b];
        let from_entities = tab(&entities, 1).unwrap();
        let pose = tab_target_pose(0.0, 0.0, 0.0, None, &[(2, 5.0, 0.0), (3, 0.0, 5.0)]).unwrap();
        assert_eq!(from_entities, pose);
    }
}
