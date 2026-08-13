//! Player death: corpse marker, PlayerDied event, combat clear.

use crate::corpse::{clear_corpse_marker_world, has_corpse_marker_world, record_corpse_world};
use crate::ecs::components::{Bags, ClassKit, Combat, Health, Motion};
use crate::ecs::World;
use woc_protocol::{EntityId, SimEvent};

/// After combat / kill rewards: finalize players who reached HP ≤ 0 this tick.
///
/// Sets `alive = false`, records corpse position, clears combat state, and emits
/// [`SimEvent::PlayerDied`] + a release toast (once per death).
pub fn on_player_death_check(world: &mut World, events: &mut Vec<SimEvent>) {
    let ids = world.ids::<ClassKit>();
    for id in ids {
        let Some(h) = world.get::<Health>(id) else {
            continue;
        };
        if h.hp > 0.0 {
            continue;
        }
        if h.alive {
            if let Some(h) = world.get_mut::<Health>(id) {
                h.alive = false;
            }
        }
        if has_corpse_marker_world(world, id) {
            continue;
        }
        finalize_player_death(world, id, events);
    }
}

fn finalize_player_death(world: &mut World, id: EntityId, events: &mut Vec<SimEvent>) {
    crate::mount::dismount(world, id, events);
    record_corpse_world(world, id);
    if let Some(c) = world.get_mut::<Combat>(id) {
        c.auto_attack = false;
        c.target = None;
        c.swing_timer = 0.0;
    }
    if let Some(bags) = world.get_mut::<Bags>(id) {
        bags.open_vendor_npc = None;
        bags.buyback.clear();
    }
    if let Some(m) = world.get_mut::<Motion>(id) {
        m.flying = false;
        m.vx = 0.0;
        m.vz = 0.0;
        m.vy = 0.0;
        m.on_ground = true;
    }
    events.push(SimEvent::PlayerDied { player: id });
    events.push(SimEvent::Toast {
        message: "You have died. Release your spirit to return to the graveyard.".into(),
    });
}

/// True when the player is dead (HP ≤ 0 / `alive == false`).
pub fn is_player_dead(world: &World, player_id: EntityId) -> bool {
    world.get::<ClassKit>(player_id).is_some()
        && world
            .get::<Health>(player_id)
            .map(|h| !h.alive)
            .unwrap_or(false)
}

/// Clear death bookkeeping after a successful spirit release.
pub fn clear_death_state(world: &mut World, player_id: EntityId) {
    clear_corpse_marker_world(world, player_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpse::has_corpse_marker_world;
    use crate::spirit::release_spirit;
    use woc_content::PlayerClass;

    #[test]
    fn hp_zero_finalizes_death_and_emits_player_died() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 10.0, -5.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 0.0;
        }
        let mut events = Vec::new();
        on_player_death_check(&mut world, &mut events);
        assert!(!world.get::<Health>(1).unwrap().alive);
        assert!(has_corpse_marker_world(&world, 1));
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::PlayerDied { player: 1 })));
        let before = events.len();
        on_player_death_check(&mut world, &mut events);
        assert_eq!(events.len(), before);
    }

    #[test]
    fn release_after_death_lands_on_eastbrook_graveyard() {
        let gy = woc_content::graveyard("eastbrook_graveyard").expect("eastbrook graveyard");
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 22.0, -20.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 0.0;
        }
        let mut events = Vec::new();
        on_player_death_check(&mut world, &mut events);
        assert!(release_spirit(&mut world, 1, &mut events));
        assert!(world.get::<Health>(1).unwrap().alive);
        let t = world.get::<crate::ecs::components::Transform>(1).unwrap();
        assert!((t.x - gy.x).abs() < 1e-5);
        assert!((t.z - gy.z).abs() < 1e-5);
        assert!(!has_corpse_marker_world(&world, 1));
    }
}
