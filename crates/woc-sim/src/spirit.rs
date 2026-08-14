//! Spirit release: respawn a dead player at a content graveyard.

use crate::corpse::clear_corpse_marker_world;
use crate::ecs::components::{ClassKit, Combat, Health, Identity, InstanceAt, Transform};
use crate::ecs::World;
use woc_content::{graveyard, graveyard_for_zone};
use woc_protocol::{EntityId, SimEvent};

/// Default graveyard when zone lookup is unavailable (Eastbrook framework slice).
const DEFAULT_GRAVEYARD_ID: &str = "eastbrook_graveyard";

/// Respawn a dead player at the Eastbrook (or zone) graveyard.
pub fn release_spirit(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    if world.get::<ClassKit>(player_id).is_none() {
        return false;
    }
    let Some(h) = world.get::<Health>(player_id) else {
        return false;
    };
    if h.alive {
        return false;
    }

    let instance_key = world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.clone());
    let parent_zone = instance_key
        .as_deref()
        .and_then(crate::instances::parent_zone_for_instance_key)
        .map(|s| s.to_string())
        .or_else(|| {
            world
                .get::<Identity>(player_id)
                .map(|i| i.zone_id.clone())
        })
        .unwrap_or_else(|| "eastbrook".into());

    if instance_key.is_some() {
        let _ = crate::instances::leave_instance(world, player_id, events);
    }

    let gy = graveyard_for_zone(&parent_zone)
        .or_else(|| graveyard(DEFAULT_GRAVEYARD_ID))
        .unwrap_or_else(|| {
            woc_content::GRAVEYARDS
                .first()
                .expect("at least one graveyard must exist in content")
        });
    let y = crate::ecs::spawn::ground_at(gy.x, gy.z);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::death::on_player_death_check;
    use woc_content::PlayerClass;

    #[test]
    fn release_spirit_noop_while_alive() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hero", PlayerClass::Warrior, 2.0, 4.0);
        let mut events = Vec::new();
        assert!(!release_spirit(&mut world, 1, &mut events));
        assert!(events.is_empty());
    }

    #[test]
    fn release_in_barrow_uses_mirefen_graveyard() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 25.0, 430.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        let def = woc_content::dungeon("mirefen_barrow").unwrap();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let parties = crate::social::party::PartyRoster::new();
        let mut events = Vec::new();
        assert!(crate::instances::enter_dungeon(
            &mut world,
            &parties,
            1,
            "mirefen_barrow",
            &mut events
        ));
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 0.0;
            h.alive = false;
        }
        assert!(release_spirit(&mut world, 1, &mut events));
        assert!(world
            .get::<crate::ecs::components::InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
        let gy = woc_content::graveyard("mirefen_graveyard").unwrap();
        let t = world.get::<Transform>(1).unwrap();
        assert!((t.x - gy.x).abs() < 1e-3);
        assert!((t.z - gy.z).abs() < 1e-3);
        assert_eq!(world.get::<crate::ecs::components::Identity>(1).unwrap().zone_id, "mirefen");
    }

    #[test]
    fn release_spirit_is_deterministic() {
        let gy = graveyard("eastbrook_graveyard").unwrap();
        let mut wa = World::new();
        let mut wb = World::new();
        crate::ecs::spawn::create_player(&mut wa, 1, "A", PlayerClass::Warrior, 30.0, -10.0);
        crate::ecs::spawn::create_player(&mut wb, 1, "A", PlayerClass::Warrior, 30.0, -10.0);
        if let Some(h) = wa.get_mut::<Health>(1) {
            h.hp = 0.0;
        }
        if let Some(h) = wb.get_mut::<Health>(1) {
            h.hp = 0.0;
        }
        let mut ea = Vec::new();
        let mut eb = Vec::new();
        on_player_death_check(&mut wa, &mut ea);
        on_player_death_check(&mut wb, &mut eb);
        assert!(release_spirit(&mut wa, 1, &mut ea));
        assert!(release_spirit(&mut wb, 1, &mut eb));
        let ta = wa.get::<Transform>(1).unwrap();
        let tb = wb.get::<Transform>(1).unwrap();
        assert!((ta.x - tb.x).abs() < 1e-5);
        assert!((ta.z - tb.z).abs() < 1e-5);
        assert!((ta.x - gy.x).abs() < 1e-5);
        assert!((ta.z - gy.z).abs() < 1e-5);
    }
}
