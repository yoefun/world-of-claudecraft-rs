//! Player death: corpse marker, PlayerDied event, combat clear.

use crate::corpse::{clear_corpse_marker, has_corpse_marker, record_corpse};
use crate::entity::Entity;
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// After combat / kill rewards: finalize players who reached HP ≤ 0 this tick.
///
/// Sets `alive = false`, records corpse position, clears combat state, and emits
/// [`SimEvent::PlayerDied`] + a release toast (once per death).
pub fn on_player_death_check(entities: &mut [Entity], events: &mut Vec<SimEvent>) {
    for e in entities.iter_mut() {
        if e.kind != EntityKind::Player {
            continue;
        }
        if e.hp > 0.0 {
            continue;
        }
        if e.alive {
            e.alive = false;
        }
        if has_corpse_marker(e) {
            continue;
        }
        finalize_player_death(e, events);
    }
}

fn finalize_player_death(player: &mut Entity, events: &mut Vec<SimEvent>) {
    record_corpse(player);
    player.auto_attack = false;
    player.target = None;
    player.swing_timer = 0.0;
    player.flying = false;
    player.vx = 0.0;
    player.vz = 0.0;
    player.vy = 0.0;
    player.on_ground = true;
    let id = player.id;
    events.push(SimEvent::PlayerDied { player: id });
    events.push(SimEvent::Toast {
        message: "You have died. Release your spirit to return to the graveyard.".into(),
    });
}

/// True when the player is dead (HP ≤ 0 / `alive == false`).
pub fn is_player_dead(entities: &[Entity], player_id: EntityId) -> bool {
    entities
        .iter()
        .find(|e| e.id == player_id && e.kind == EntityKind::Player)
        .map(|e| !e.alive)
        .unwrap_or(false)
}

/// Clear death bookkeeping after a successful spirit release.
pub fn clear_death_state(player: &mut Entity) {
    clear_corpse_marker(player);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use crate::spirit::release_spirit;
    use woc_content::PlayerClass;

    #[test]
    fn hp_zero_finalizes_death_and_emits_player_died() {
        let mut entities = vec![create_player(1, "Hero", PlayerClass::Warrior, 10.0, -5.0)];
        entities[0].hp = 0.0;
        let mut events = Vec::new();
        on_player_death_check(&mut entities, &mut events);
        assert!(!entities[0].alive);
        assert!(has_corpse_marker(&entities[0]));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::PlayerDied { player: 1 })));
        // Idempotent: no second PlayerDied.
        let before = events.len();
        on_player_death_check(&mut entities, &mut events);
        assert_eq!(events.len(), before);
    }

    #[test]
    fn release_after_death_lands_on_eastbrook_graveyard() {
        let gy = woc_content::graveyard("eastbrook_graveyard").expect("eastbrook graveyard");
        let mut entities = vec![create_player(1, "Hero", PlayerClass::Warrior, 22.0, -20.0)];
        entities[0].hp = 0.0;
        let mut events = Vec::new();
        on_player_death_check(&mut entities, &mut events);
        assert!(release_spirit(&mut entities, 1, &mut events));
        assert!(entities[0].alive);
        assert!((entities[0].x - gy.x).abs() < 1e-5);
        assert!((entities[0].z - gy.z).abs() < 1e-5);
        assert!(!has_corpse_marker(&entities[0]));
    }
}
