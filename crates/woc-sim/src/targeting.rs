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
    use crate::ecs::components::{Combat, Health, Identity, Transform};
    use crate::sim::Sim;
    use woc_content::PlayerClass;
    use woc_protocol::EntityKind;

    #[test]
    fn tab_cycles_at_least_two_mobs() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 5.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 0.0, 5.0).unwrap();
        let first = tab_target(&world, 1).expect("first");
        assert!(first == 2 || first == 3);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(first);
        }
        let second = tab_target(&world, 1).expect("second");
        assert_ne!(second, first);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(second);
        }
        assert_eq!(tab_target(&world, 1).unwrap(), first);
    }

    #[test]
    fn tab_ignores_dead_mobs() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 3.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 0.0, 3.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(2) {
            h.alive = false;
        }
        assert_eq!(tab_target(&world, 1), Some(3));
    }

    #[test]
    fn sim_tab_target_and_clear() {
        let mut sim = Sim::new_eastbrook("Tabber", PlayerClass::Warrior);
        let (px, pz) = {
            let t = sim.world.get::<Transform>(sim.player_id).unwrap();
            (t.x, t.z)
        };
        let mobs: Vec<_> = sim
            .world
            .live_ids()
            .filter(|&id| {
                sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                    && sim
                        .world
                        .get::<Health>(id)
                        .map(|h| h.alive)
                        .unwrap_or(false)
            })
            .take(2)
            .collect();
        assert!(mobs.len() >= 2);
        for (i, id) in mobs.iter().enumerate() {
            if let Some(t) = sim.world.get_mut::<Transform>(*id) {
                t.x = px + 2.0 + i as f32;
                t.z = pz;
                t.y = crate::ecs::spawn::ground_at(t.x, t.z);
            }
        }
        let first = sim.tab_target().expect("tab");
        assert_eq!(
            sim.world.get::<Combat>(sim.player_id).unwrap().target,
            Some(first)
        );
        let second = sim.tab_target().expect("cycle");
        assert_ne!(first, second);
        if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
            c.auto_attack = true;
        }
        sim.clear_target();
        let c = sim.world.get::<Combat>(sim.player_id).unwrap();
        assert!(c.target.is_none());
        assert!(!c.auto_attack);
    }

    #[test]
    fn tab_target_pose_matches_entity_tab() {
        let pose = tab_target_pose(0.0, 0.0, 0.0, None, &[(2, 5.0, 0.0), (3, 0.0, 5.0)]).unwrap();
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 5.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 0.0, 5.0).unwrap();
        assert_eq!(tab_target(&world, 1).unwrap(), pose);
    }
}
