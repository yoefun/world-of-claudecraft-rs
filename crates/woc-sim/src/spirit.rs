//! Spirit release: respawn a dead player at a content graveyard.

use crate::corpse::clear_corpse_marker;
use crate::entity::Entity;
use woc_content::{graveyard, graveyard_for_zone, GraveyardDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Default graveyard when zone lookup is unavailable (Eastbrook framework slice).
const DEFAULT_GRAVEYARD_ID: &str = "eastbrook_graveyard";

/// Respawn a dead player at the Eastbrook (or zone) graveyard.
///
/// Returns `false` if the player is missing, not a player, or still alive.
pub fn release_spirit(
    entities: &mut [Entity],
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return false;
    };
    if entities[pi].kind != EntityKind::Player || entities[pi].alive {
        return false;
    }

    let gy = resolve_graveyard();
    entities[pi].x = gy.x;
    entities[pi].z = gy.z;
    entities[pi].y = Entity::ground_at(gy.x, gy.z);
    entities[pi].hp = entities[pi].hp_max;
    entities[pi].alive = true;
    entities[pi].auto_attack = false;
    entities[pi].target = None;
    entities[pi].swing_timer = 0.0;
    clear_corpse_marker(&mut entities[pi]);

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

    #[test]
    fn release_spirit_noop_while_alive() {
        let mut entities = vec![create_player(1, "Hero", PlayerClass::Warrior, 2.0, 4.0)];
        let mut events = Vec::new();
        assert!(!release_spirit(&mut entities, 1, &mut events));
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
        on_player_death_check(&mut a, &mut ea);
        on_player_death_check(&mut b, &mut eb);
        assert!(release_spirit(&mut a, 1, &mut ea));
        assert!(release_spirit(&mut b, 1, &mut eb));
        assert!((a[0].x - b[0].x).abs() < 1e-5);
        assert!((a[0].z - b[0].z).abs() < 1e-5);
        assert!((a[0].x - gy.x).abs() < 1e-5);
        assert!((a[0].z - gy.z).abs() < 1e-5);
    }
}
