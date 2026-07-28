//! Dedicated multi-room delve lifecycle.

use crate::entity::{create_mob_from_template, grant_into, Entity};
use crate::zones::load_overworld_zone;
use woc_content::{delve, mob, DelveDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Enter a content-defined delve and spawn its first room.
pub fn enter_delve(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    delve_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = delve(delve_id) else {
        return false;
    };
    let Some(player) = entities
        .iter()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
    else {
        return false;
    };
    if player.level < def.min_level
        || player.instance_id.is_some()
        || def.rooms.is_empty()
        || def
            .rooms
            .iter()
            .any(|room| room.count == 0 || mob(room.mob_template).is_none())
    {
        return false;
    }

    let mut next_id = next_entity_id(entities);
    entities.retain(|entity| {
        !matches!(
            entity.kind,
            EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
        )
    });

    let instance_zone = format!("delve:{}", def.id);
    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("validated player must survive delve cleanup");
    player.x = def.entrance_x;
    player.z = def.entrance_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.zone_id = instance_zone.clone();
    player.instance_id = Some(def.id.to_string());
    player.delve_room = Some(0);
    reset_combat_state(player);

    spawn_room(entities, &mut next_id, def, 0, &instance_zone);
    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    true
}

/// Advance after every mob in the current room is dead, or finish the delve.
pub fn try_advance_delve(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some((delve_id, room_index)) = entities
        .iter()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
        .and_then(|player| Some((player.instance_id.clone()?, player.delve_room? as usize)))
    else {
        return false;
    };
    let Some(def) = delve(&delve_id) else {
        return false;
    };
    if room_index >= def.rooms.len()
        || entities.iter().any(|entity| {
            entity.kind == EntityKind::Mob
                && entity.alive
                && entity.instance_id.as_deref() == Some(delve_id.as_str())
        })
    {
        return false;
    }

    events.push(SimEvent::DelveRoomCleared {
        player: player_id,
        delve_id: delve_id.clone(),
        room: room_index as u32,
    });

    let next_room = room_index + 1;
    if next_room < def.rooms.len() {
        entities.retain(|entity| !matches!(entity.kind, EntityKind::Mob | EntityKind::Loot));
        let mut next_id = next_entity_id(entities);
        let instance_zone = format!("delve:{}", def.id);
        let player = entities
            .iter_mut()
            .find(|entity| entity.id == player_id)
            .expect("active delve player must exist");
        player.delve_room = Some(next_room as u32);
        player.x = def.entrance_x;
        player.z = def.entrance_z + next_room as f32 * 10.0;
        player.y = Entity::ground_at(player.x, player.z);
        reset_combat_state(player);
        spawn_room(entities, &mut next_id, def, next_room, &instance_zone);
        return true;
    }

    let mut granted_item = None;
    {
        let player = entities
            .iter_mut()
            .find(|entity| entity.id == player_id)
            .expect("active delve player must exist");
        player.copper = player.copper.saturating_add(def.reward.copper);
        if let Some(item_id) = def.reward.item_id {
            if grant_into(&mut player.inventory, item_id, def.reward.item_count) {
                granted_item = Some(item_id.to_string());
                events.push(SimEvent::ItemGained {
                    player: player_id,
                    item_id: item_id.to_string(),
                    count: def.reward.item_count,
                });
            } else {
                events.push(SimEvent::Toast {
                    message: "Inventory full; delve item reward was not granted.".into(),
                });
            }
        }
    }

    if !load_overworld_zone(entities, player_id, def.zone_id) {
        return false;
    }
    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("completed delve player must survive zone load");
    player.delve_room = None;

    events.push(SimEvent::DelveCompleted {
        player: player_id,
        delve_id,
        reward_copper: def.reward.copper,
        reward_item: granted_item,
    });
    events.push(SimEvent::InstanceLeft { player: player_id });
    true
}

fn spawn_room(
    entities: &mut Vec<Entity>,
    next_id: &mut EntityId,
    def: &DelveDef,
    room_index: usize,
    zone_id: &str,
) {
    let room = &def.rooms[room_index];
    for mob_index in 0..room.count {
        let x = def.entrance_x + 4.0 + mob_index as f32 * 2.5;
        let z = def.entrance_z + room_index as f32 * 10.0;
        let Some(mut entity) = create_mob_from_template(*next_id, room.mob_template, x, z) else {
            continue;
        };
        *next_id = next_id.saturating_add(1);
        entity.zone_id = zone_id.to_string();
        entity.instance_id = Some(def.id.to_string());
        entities.push(entity);
    }
}

fn next_entity_id(entities: &[Entity]) -> EntityId {
    entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn reset_combat_state(player: &mut Entity) {
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.cast = None;
    player.threat.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{count_item, create_mob_from_template, create_player};
    use woc_content::PlayerClass;
    use woc_protocol::{EntityKind, InteractAction, SimEvent, WorldHost};

    fn defeat_current_room(entities: &mut [crate::entity::Entity]) {
        for entity in entities.iter_mut().filter(|entity| {
            entity.kind == EntityKind::Mob
                && entity.instance_id.as_deref() == Some("eastbrook_hollow")
        }) {
            entity.alive = false;
            entity.hp = 0.0;
        }
    }

    #[test]
    fn enter_clear_advance_and_complete_grants_final_reward() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 5.0, 5.0);
        let world_mob = create_mob_from_template(2, "young_boar", 6.0, 5.0).expect("world mob");
        let mut entities = vec![player, world_mob];
        let mut events = Vec::new();

        assert!(enter_delve(
            &mut entities,
            1,
            "eastbrook_hollow",
            &mut events
        ));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.instance_id.as_deref(), Some("eastbrook_hollow"));
        assert_eq!(player.zone_id, "delve:eastbrook_hollow");
        assert_eq!(player.delve_room, Some(0));
        assert_eq!(
            entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::Mob && entity.alive)
                .count(),
            2
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::InstanceEntered { player: 1, dungeon_id }
                if dungeon_id == "eastbrook_hollow"
        )));

        defeat_current_room(&mut entities);
        assert!(try_advance_delve(&mut entities, 1, &mut events));
        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.delve_room, Some(1));
        assert_eq!(
            entities
                .iter()
                .filter(|entity| {
                    entity.kind == EntityKind::Mob
                        && entity.alive
                        && entity.instance_id.as_deref() == Some("eastbrook_hollow")
                })
                .count(),
            3
        );

        defeat_current_room(&mut entities);
        assert!(try_advance_delve(&mut entities, 1, &mut events));
        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.delve_room, Some(2));
        assert_eq!(
            entities
                .iter()
                .filter(|entity| {
                    entity.kind == EntityKind::Mob
                        && entity.alive
                        && entity.instance_id.as_deref() == Some("eastbrook_hollow")
                })
                .count(),
            1
        );

        defeat_current_room(&mut entities);
        assert!(try_advance_delve(&mut entities, 1, &mut events));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.instance_id, None);
        assert_eq!(player.delve_room, None);
        assert_eq!(player.zone_id, "eastbrook");
        assert_eq!(player.copper, 75);
        assert_eq!(count_item(&player.inventory, "eastbrook_greaves"), 1);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, SimEvent::DelveRoomCleared { .. }))
                .count(),
            3
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::DelveCompleted {
                player: 1,
                delve_id,
                reward_copper: 75,
                reward_item: Some(item),
            } if delve_id == "eastbrook_hollow" && item == "eastbrook_greaves"
        )));
    }

    #[test]
    fn cannot_advance_while_current_room_has_living_mobs() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 5.0, 5.0);
        let mut entities = vec![player];
        let mut events = Vec::new();
        assert!(enter_delve(
            &mut entities,
            1,
            "eastbrook_hollow",
            &mut events
        ));
        events.clear();

        assert!(!try_advance_delve(&mut entities, 1, &mut events));
        assert_eq!(entities[0].delve_room, Some(0));
        assert!(events.is_empty());
    }

    #[test]
    fn world_host_dispatches_enter_and_advance_actions() {
        let mut sim = crate::Sim::new_eastbrook("Delver", PlayerClass::Warrior);
        let player_id = sim.player_id;

        WorldHost::interact(
            &mut sim,
            player_id,
            0,
            InteractAction::EnterDelve {
                delve_id: "eastbrook_hollow".into(),
            },
        );
        assert_eq!(
            sim.entities
                .iter()
                .find(|entity| entity.id == player_id)
                .unwrap()
                .delve_room,
            Some(0)
        );
        assert!(sim.next_id > sim.entities.iter().map(|entity| entity.id).max().unwrap());

        defeat_current_room(&mut sim.entities);
        WorldHost::interact(&mut sim, player_id, 0, InteractAction::AdvanceDelve);
        assert_eq!(
            sim.entities
                .iter()
                .find(|entity| entity.id == player_id)
                .unwrap()
                .delve_room,
            Some(1)
        );
        assert!(sim.next_id > sim.entities.iter().map(|entity| entity.id).max().unwrap());
    }
}
