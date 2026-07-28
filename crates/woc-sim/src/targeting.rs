//! Tab-target cycling among living hostile mobs.

use crate::entity::Entity;
use woc_protocol::{EntityId, EntityKind};

/// Max range for tab-target candidates (same ballpark as combat acquire).
pub const TAB_TARGET_RANGE: f32 = 40.0;

fn angle_to(player: &Entity, target: &Entity) -> f32 {
    let dx = target.x - player.x;
    let dz = target.z - player.z;
    let facing_to = dz.atan2(dx);
    let mut delta = facing_to - player.yaw;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta.abs()
}

fn dist2d(a: &Entity, b: &Entity) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Cycle living hostile mobs by facing angle, then distance.
///
/// If the player already has a candidate targeted, returns the next in the
/// sorted cycle; otherwise returns the best (smallest angle, then nearest).
pub fn tab_target(player_id: EntityId, entities: &[Entity]) -> Option<EntityId> {
    let player = entities.iter().find(|e| e.id == player_id)?;
    if !player.alive {
        return None;
    }

    let mut candidates: Vec<(EntityId, f32, f32)> = entities
        .iter()
        .filter(|e| e.kind == EntityKind::Mob && e.alive)
        .filter_map(|e| {
            let d = dist2d(player, e);
            if d > TAB_TARGET_RANGE {
                return None;
            }
            Some((e.id, angle_to(player, e), d))
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });

    if let Some(cur) = player.target {
        if let Some(idx) = candidates.iter().position(|(id, ..)| *id == cur) {
            let next = (idx + 1) % candidates.len();
            return Some(candidates[next].0);
        }
    }
    Some(candidates[0].0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player};
    use woc_content::PlayerClass;

    #[test]
    fn tab_cycles_at_least_two_mobs() {
        let mut player = create_player(1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        player.yaw = 0.0;

        let mut a = create_mob_from_template(2, "young_wolf", 5.0, 0.0).unwrap();
        let mut b = create_mob_from_template(3, "young_wolf", 0.0, 5.0).unwrap();
        a.alive = true;
        b.alive = true;

        let entities = vec![player.clone(), a, b];
        let first = tab_target(1, &entities).expect("first target");
        assert!(first == 2 || first == 3);

        let mut entities2 = entities;
        entities2[0].target = Some(first);
        let second = tab_target(1, &entities2).expect("second target");
        assert_ne!(second, first);
        assert!(second == 2 || second == 3);

        entities2[0].target = Some(second);
        let third = tab_target(1, &entities2).expect("cycles back");
        assert_eq!(third, first);
    }

    #[test]
    fn tab_ignores_dead_mobs() {
        let player = create_player(1, "Tabber", PlayerClass::Warrior, 0.0, 0.0);
        let mut dead = create_mob_from_template(2, "young_wolf", 3.0, 0.0).unwrap();
        dead.alive = false;
        let live = create_mob_from_template(3, "young_wolf", 0.0, 3.0).unwrap();
        let entities = vec![player, dead, live];
        assert_eq!(tab_target(1, &entities), Some(3));
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

        sim.clear_target();
        assert!(sim.player().unwrap().target.is_none());
    }
}
