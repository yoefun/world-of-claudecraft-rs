//! Spirit release: respawn a dead player at a content graveyard.

use crate::corpse::clear_corpse_marker_world;
use crate::ecs::components::{ClassKit, Combat, Health, Transform};
use crate::ecs::World;
use crate::entity::Entity;
use woc_content::{graveyard, graveyard_for_zone, GraveyardDef};
use woc_protocol::{EntityId, SimEvent};

/// Default graveyard when zone lookup is unavailable (Eastbrook framework slice).
const DEFAULT_GRAVEYARD_ID: &str = "eastbrook_graveyard";

/// Respawn a dead player at the Eastbrook (or zone) graveyard.
///
/// Returns `false` if the player is missing, not a player, or still alive.
pub fn release_spirit(
    world: &mut World,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    if world.get::<ClassKit>(player_id).is_none() {
        return false;
    }
    let Some(h) = world.get::<Health>(player_id) else {
        return false;
    };
    if h.alive {
        return false;
    }

    let gy = resolve_graveyard();
    let y = Entity::ground_at(gy.x, gy.z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = gy.x;
        t.z = gy.z;
        t.y = y;
    }
    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.hp = h.hp_max;
        h.alive = true;
    }
    if let Some(c) = world.get_mut::<Combat>(player_id) {
        c.auto_attack = false;
        c.target = None;
        c.swing_timer = 0.0;
    }
    clear_corpse_marker_world(world, player_id);

    events.push(SimEvent::Toast {
        message: format!("You return to life at {}.", gy.id.replace('_', " ")),
    });
    true
}

fn resolve_graveyard() -> &'static GraveyardDef {
    graveyard_for_zone("eastbrook")
        .or_else(|| graveyard(DEFAULT_GRAVEYARD_ID))
        .unwrap_or_else(|| {
            woc_content::GRAVEYARDS
                .first()
                .expect("at least one graveyard must exist in content")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::death::on_player_death_check;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    fn run_death_then_release(
        entities: &mut [Entity],
        player_id: EntityId,
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let mut world = crate::ecs::spawn::world_from_entities(entities);
        on_player_death_check(&mut world, events);
        let ok = release_spirit(&mut world, player_id, events);
        crate::ecs::spawn::apply_world_to_entities(&world, entities);
        ok
    }

    #[test]
    fn release_spirit_noop_while_alive() {
        let mut entities = vec![create_player(1, "Hero", PlayerClass::Warrior, 2.0, 4.0)];
        let mut events = Vec::new();
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        assert!(!release_spirit(&mut world, 1, &mut events));
        crate::ecs::spawn::apply_world_to_entities(&world, &mut entities);
        assert!(events.is_empty());
    }

    #[test]
    fn release_spirit_is_deterministic() {
        let gy = graveyard("eastbrook_graveyard").unwrap();
        let mut a = vec![create_player(1, "A", PlayerClass::Warrior, 30.0, -10.0)];
        let mut b = vec![create_player(1, "A", PlayerClass::Warrior, 30.0, -10.0)];
        a[0].hp = 0.0;
        b[0].hp = 0.0;
        let mut ea = Vec::new();
        let mut eb = Vec::new();
        assert!(run_death_then_release(&mut a, 1, &mut ea));
        assert!(run_death_then_release(&mut b, 1, &mut eb));
        assert!((a[0].x - b[0].x).abs() < 1e-5);
        assert!((a[0].z - b[0].z).abs() < 1e-5);
        assert!((a[0].x - gy.x).abs() < 1e-5);
        assert!((a[0].z - gy.z).abs() < 1e-5);
    }
}
