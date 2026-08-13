//! Dedicated multi-room delve lifecycle.

use crate::ecs::components::{
    Bags, Combat, Health, Identity, InstanceAt, Progress, Threat, Transform,
};
use crate::ecs::World;
use crate::entity::{create_mob_from_template, grant_into, Entity};
use crate::zones::load_overworld_zone;
use woc_content::{delve, mob, DelveDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Enter a content-defined delve and spawn its first room.
pub fn enter_delve(
    world: &mut World,
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    delve_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = delve(delve_id) else {
        return false;
    };
    if world
        .get::<Identity>(player_id)
        .map(|i| i.kind)
        != Some(EntityKind::Player)
    {
        return false;
    }
    let level = world
        .get::<Health>(player_id)
        .map(|h| h.level)
        .unwrap_or(1);
    let in_instance = world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.as_ref())
        .is_some();
    if level < def.min_level
        || in_instance
        || def.rooms.is_empty()
        || def
            .rooms
            .iter()
            .any(|room| room.count == 0 || mob(room.mob_template).is_none())
    {
        return false;
    }

    let to_remove: Vec<EntityId> = world
        .live_ids()
        .filter(|&id| {
            world.get::<Identity>(id).is_some_and(|identity| {
                matches!(
                    identity.kind,
                    EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
                )
            })
        })
        .collect();
    for id in to_remove {
        world.despawn(id);
    }
    entities.retain(|entity| {
        !matches!(
            entity.kind,
            EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
        )
    });

    let mut next_id = next_entity_id(entities, world);
    let instance_zone = format!("delve:{}", def.id);
    let spawn_y = Entity::ground_at(def.entrance_x, def.entrance_z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = def.entrance_x;
        t.z = def.entrance_z;
        t.y = spawn_y;
    }
    if let Some(identity) = world.get_mut::<Identity>(player_id) {
        identity.zone_id = instance_zone.clone();
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.instance_id = Some(def.id.to_string());
        inst.delve_room = Some(0);
    }
    reset_combat_state(world, player_id);

    spawn_room(world, entities, &mut next_id, def, 0, &instance_zone);
    world.set_next_id(next_id);

    if let Some(entity) = entities.iter_mut().find(|e| e.id == player_id) {
        crate::ecs::spawn::apply_world_to_entity(world, entity);
    }

    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    true
}

/// Advance after every mob in the current room is dead, or finish the delve.
pub fn try_advance_delve(
    world: &mut World,
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some((delve_id, room_index)) = world
        .get::<InstanceAt>(player_id)
        .and_then(|player| Some((player.instance_id.clone()?, player.delve_room? as usize)))
    else {
        return false;
    };
    let Some(def) = delve(&delve_id) else {
        return false;
    };
    if room_index >= def.rooms.len()
        || world.ids::<Identity>().into_iter().any(|id| {
            world.get::<Identity>(id).is_some_and(|identity| {
                identity.kind == EntityKind::Mob
                    && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
                    && world
                        .get::<InstanceAt>(id)
                        .and_then(|i| i.instance_id.as_deref())
                        == Some(delve_id.as_str())
            })
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
        let to_remove: Vec<EntityId> = world
            .live_ids()
            .filter(|&id| {
                world.get::<Identity>(id).is_some_and(|identity| {
                    matches!(identity.kind, EntityKind::Mob | EntityKind::Loot)
                })
            })
            .collect();
        for id in to_remove {
            world.despawn(id);
        }
        entities.retain(|entity| !matches!(entity.kind, EntityKind::Mob | EntityKind::Loot));

        let mut next_id = next_entity_id(entities, world);
        let instance_zone = format!("delve:{}", def.id);
        let spawn_y = Entity::ground_at(def.entrance_x, def.entrance_z + next_room as f32 * 10.0);
        if let Some(t) = world.get_mut::<Transform>(player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z + next_room as f32 * 10.0;
            t.y = spawn_y;
        }
        if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
            inst.delve_room = Some(next_room as u32);
        }
        reset_combat_state(world, player_id);
        spawn_room(world, entities, &mut next_id, def, next_room, &instance_zone);
        world.set_next_id(next_id);

        if let Some(entity) = entities.iter_mut().find(|e| e.id == player_id) {
            crate::ecs::spawn::apply_world_to_entity(world, entity);
        }
        return true;
    }

    let mut granted_item = None;
    if let Some(progress) = world.get_mut::<Progress>(player_id) {
        progress.copper = progress.copper.saturating_add(def.reward.copper);
    }
    if let Some(item_id) = def.reward.item_id {
        if let Some(bags) = world.get_mut::<crate::ecs::components::Bags>(player_id) {
            if grant_into(&mut bags.inventory, item_id, def.reward.item_count) {
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

    if !load_overworld_zone(world, entities, player_id, def.zone_id) {
        return false;
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.delve_room = None;
    }
    if let Some(entity) = entities.iter_mut().find(|e| e.id == player_id) {
        crate::ecs::spawn::apply_world_to_entity(world, entity);
    }

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
    world: &mut World,
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
        crate::ecs::spawn::sync_entity_to_world(world, &entity);
        entities.push(entity);
    }
}

fn next_entity_id(entities: &[Entity], world: &World) -> EntityId {
    entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(world.next_id())
}

fn reset_combat_state(world: &mut World, player_id: EntityId) {
    if let Some(combat) = world.get_mut::<Combat>(player_id) {
        combat.target = None;
        combat.auto_attack = false;
        combat.cast = None;
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.open_vendor_npc = None;
    }
    if let Some(threat) = world.get_mut::<Threat>(player_id) {
        threat.threat.clear();
    }
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
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        let mut events = Vec::new();

        assert!(enter_delve(
            &mut world,
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
        for entity in &entities {
            crate::ecs::spawn::sync_entity_to_world(&mut world, entity);
        }
        assert!(try_advance_delve(&mut world, &mut entities, 1, &mut events));
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
        for entity in &entities {
            crate::ecs::spawn::sync_entity_to_world(&mut world, entity);
        }
        assert!(try_advance_delve(&mut world, &mut entities, 1, &mut events));
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
        for entity in &entities {
            crate::ecs::spawn::sync_entity_to_world(&mut world, entity);
        }
        assert!(try_advance_delve(&mut world, &mut entities, 1, &mut events));

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
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        let mut events = Vec::new();
        assert!(enter_delve(
            &mut world,
            &mut entities,
            1,
            "eastbrook_hollow",
            &mut events
        ));
        events.clear();

        assert!(!try_advance_delve(&mut world, &mut entities, 1, &mut events));
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
        sim.rebuild_world();
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
