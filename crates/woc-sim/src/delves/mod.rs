//! Dedicated multi-room delve lifecycle.

use crate::ecs::components::{
    Bags, Combat, Health, Identity, InstanceAt, Progress, Threat, Transform,
};
use crate::ecs::World;
use crate::instances::{
    dungeon_id_from_instance, follow_owner_into_instance, INSTANCE_ENTER_RANGE,
};
use crate::inventory::grant_into;
use woc_content::{delve, mob, DelveDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Enter a content-defined delve and spawn its first room.
pub fn enter_delve(
    world: &mut World,
    player_id: EntityId,
    delve_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = delve(delve_id) else {
        events.push(SimEvent::Toast {
            message: "There is no such instance.".into(),
        });
        return false;
    };
    if world.get::<Identity>(player_id).map(|i| i.kind) != Some(EntityKind::Player) {
        return false;
    }
    let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
    let in_instance = world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.as_ref())
        .is_some();
    if in_instance {
        events.push(SimEvent::Toast {
            message: "You are already in an instance.".into(),
        });
        return false;
    }
    if level < def.min_level {
        events.push(SimEvent::Toast {
            message: format!("You must be level {} to enter {}.", def.min_level, def.name),
        });
        return false;
    }
    let distance = world
        .get::<Transform>(player_id)
        .map(|transform| {
            let dx = transform.x - def.entrance_x;
            let dz = transform.z - def.entrance_z;
            (dx * dx + dz * dz).sqrt()
        })
        .unwrap_or(f32::MAX);
    if distance > INSTANCE_ENTER_RANGE {
        events.push(SimEvent::Toast {
            message: "You must be closer to the entrance.".into(),
        });
        return false;
    }
    if def.rooms.is_empty()
        || def
            .rooms
            .iter()
            .any(|room| room.count == 0 || mob(room.mob_template).is_none())
    {
        return false;
    }

    let seq = world.next_id();
    world.set_next_id(seq.saturating_add(1));
    let instance_key = format!("{}#{}", def.id, seq);
    let instance_zone = format!("delve:{}", def.id);
    let spawn_y = crate::ecs::spawn::ground_at(def.entrance_x, def.entrance_z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = def.entrance_x;
        t.z = def.entrance_z;
        t.y = spawn_y;
    }
    if let Some(identity) = world.get_mut::<Identity>(player_id) {
        identity.zone_id = instance_zone.clone();
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.instance_id = Some(instance_key.clone());
        inst.delve_room = Some(0);
    }
    follow_owner_into_instance(world, player_id);
    crate::mount::dismount(world, player_id, events);
    reset_combat_state(world, player_id);

    spawn_room(world, def, 0, &instance_zone, &instance_key);

    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Entered {}.", def.name),
    });
    true
}

/// Advance after every mob in the current room is dead, or finish the delve.
pub fn try_advance_delve(
    world: &mut World,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some((instance_key, room_index)) = world
        .get::<InstanceAt>(player_id)
        .and_then(|player| Some((player.instance_id.clone()?, player.delve_room? as usize)))
    else {
        return false;
    };
    let Some(def) = delve(dungeon_id_from_instance(&instance_key)) else {
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
                        == Some(instance_key.as_str())
            })
        })
    {
        return false;
    }

    events.push(SimEvent::DelveRoomCleared {
        player: player_id,
        delve_id: def.id.to_string(),
        room: room_index as u32,
    });

    let next_room = room_index + 1;
    if next_room < def.rooms.len() {
        let to_remove: Vec<EntityId> = world
            .live_ids()
            .filter(|&id| {
                world.get::<Identity>(id).is_some_and(|identity| {
                    matches!(identity.kind, EntityKind::Mob | EntityKind::Loot)
                        && world
                            .get::<InstanceAt>(id)
                            .and_then(|i| i.instance_id.as_deref())
                            == Some(instance_key.as_str())
                })
            })
            .collect();
        for id in to_remove {
            world.despawn(id);
        }

        let instance_zone = format!("delve:{}", def.id);
        let spawn_y =
            crate::ecs::spawn::ground_at(def.entrance_x, def.entrance_z + next_room as f32 * 10.0);
        if let Some(t) = world.get_mut::<Transform>(player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z + next_room as f32 * 10.0;
            t.y = spawn_y;
        }
        if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
            inst.delve_room = Some(next_room as u32);
        }
        reset_combat_state(world, player_id);
        spawn_room(world, def, next_room, &instance_zone, &instance_key);
        return true;
    }

    let mut granted_item = None;
    if let Some(progress) = world.get_mut::<Progress>(player_id) {
        progress.copper = progress.copper.saturating_add(def.reward.copper);
    }
    if let Some(item_id) = def.reward.item_id {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
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

    events.push(SimEvent::DelveCompleted {
        player: player_id,
        delve_id: def.id.to_string(),
        reward_copper: def.reward.copper,
        reward_item: granted_item,
    });
    crate::instances::leave_instance(world, player_id, events)
}

fn spawn_room(
    world: &mut World,
    def: &DelveDef,
    room_index: usize,
    zone_id: &str,
    instance_key: &str,
) {
    let room = &def.rooms[room_index];
    for mob_index in 0..room.count {
        let x = def.entrance_x + 4.0 + mob_index as f32 * 2.5;
        let z = def.entrance_z + room_index as f32 * 10.0;
        let id = world.next_id();
        let Some(mid) =
            crate::ecs::spawn::create_mob_from_template(world, id, room.mob_template, x, z)
        else {
            continue;
        };
        if let Some(identity) = world.get_mut::<Identity>(mid) {
            identity.zone_id = zone_id.to_string();
        }
        if let Some(inst) = world.get_mut::<InstanceAt>(mid) {
            inst.instance_id = Some(instance_key.to_string());
        }
    }
}

fn reset_combat_state(world: &mut World, player_id: EntityId) {
    if let Some(combat) = world.get_mut::<Combat>(player_id) {
        combat.target = None;
        combat.auto_attack = false;
        combat.cast = None;
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.open_vendor_npc = None;
        bags.buyback.clear();
    }
    if let Some(threat) = world.get_mut::<Threat>(player_id) {
        threat.threat.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::count_item;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, WorldHost};

    fn defeat_current_room(world: &mut World, instance_key: &str) {
        let ids: Vec<_> = world
            .ids::<Identity>()
            .into_iter()
            .filter(|&id| {
                world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                    && world
                        .get::<InstanceAt>(id)
                        .and_then(|i| i.instance_id.as_deref())
                        == Some(instance_key)
            })
            .collect();
        for id in ids {
            if let Some(h) = world.get_mut::<Health>(id) {
                h.alive = false;
                h.hp = 0.0;
            }
        }
    }

    fn living_delve_mobs(world: &World, instance_key: &str) -> usize {
        world
            .ids::<Identity>()
            .into_iter()
            .filter(|&id| {
                world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                    && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
                    && world
                        .get::<InstanceAt>(id)
                        .and_then(|i| i.instance_id.as_deref())
                        == Some(instance_key)
            })
            .count()
    }

    #[test]
    fn enter_clear_advance_and_complete_grants_final_reward() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 5.0, 5.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_boar", 6.0, 5.0)
            .expect("world mob");
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let mut events = Vec::new();

        assert!(enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));

        assert!(world.get::<Health>(2).is_some_and(|health| health.alive));
        let instance_key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .expect("delve instance key");
        assert!(instance_key.starts_with("eastbrook_hollow#"));
        assert_eq!(
            world.get::<Identity>(1).unwrap().zone_id,
            "delve:eastbrook_hollow"
        );
        assert_eq!(world.get::<InstanceAt>(1).unwrap().delve_room, Some(0));
        assert_eq!(living_delve_mobs(&world, &instance_key), 2);
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::InstanceEntered { player: 1, dungeon_id }
                if dungeon_id == "eastbrook_hollow"
        )));

        defeat_current_room(&mut world, &instance_key);
        assert!(try_advance_delve(&mut world, 1, &mut events));
        assert_eq!(world.get::<InstanceAt>(1).unwrap().delve_room, Some(1));
        assert_eq!(living_delve_mobs(&world, &instance_key), 3);

        defeat_current_room(&mut world, &instance_key);
        assert!(try_advance_delve(&mut world, 1, &mut events));
        assert_eq!(world.get::<InstanceAt>(1).unwrap().delve_room, Some(2));
        assert_eq!(living_delve_mobs(&world, &instance_key), 1);

        defeat_current_room(&mut world, &instance_key);
        assert!(try_advance_delve(&mut world, 1, &mut events));

        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
        assert_eq!(world.get::<InstanceAt>(1).unwrap().delve_room, None);
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "eastbrook");
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 75);
        assert_eq!(
            count_item(
                &world.get::<Bags>(1).unwrap().inventory,
                "eastbrook_greaves"
            ),
            1
        );
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
    fn two_players_get_distinct_hollow_keys() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "A", PlayerClass::Warrior, 8.0, -6.0);
        crate::ecs::spawn::create_player(&mut world, 2, "B", PlayerClass::Mage, 8.0, -6.0);
        // 1.23 moves entrance to (8,-6); until Task 9 the table is still (0,0).
        // Place both on the *current* table entrance so this task stays green:
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        for id in [1, 2] {
            if let Some(t) = world.get_mut::<Transform>(id) {
                t.x = def.entrance_x;
                t.z = def.entrance_z;
            }
        }
        let mut events = Vec::new();
        assert!(enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        assert!(enter_delve(&mut world, 2, "eastbrook_hollow", &mut events));
        let a = world
            .get::<InstanceAt>(1)
            .unwrap()
            .instance_id
            .clone()
            .unwrap();
        let b = world
            .get::<InstanceAt>(2)
            .unwrap()
            .instance_id
            .clone()
            .unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("eastbrook_hollow#"));
        assert!(b.starts_with("eastbrook_hollow#"));
    }

    #[test]
    fn enter_delve_rejects_when_too_far() {
        let mut world = World::new();
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        crate::ecs::spawn::create_player(
            &mut world,
            1,
            "Far",
            PlayerClass::Warrior,
            def.entrance_x + INSTANCE_ENTER_RANGE + 1.0,
            def.entrance_z,
        );
        let mut events = Vec::new();
        assert!(!enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You must be closer to the entrance."
        )));
    }

    #[test]
    fn leaving_delve_early_grants_no_reward() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Aborter", PlayerClass::Warrior, 0.0, 0.0);
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let mut events = Vec::new();

        assert!(enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        assert!(crate::instances::leave_instance(&mut world, 1, &mut events));

        assert_eq!(world.get::<Progress>(1).unwrap().copper, 0);
        assert_eq!(
            count_item(
                &world.get::<Bags>(1).unwrap().inventory,
                "eastbrook_greaves"
            ),
            0
        );
    }

    #[test]
    fn cannot_advance_while_current_room_has_living_mobs() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 5.0, 5.0);
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        let mut events = Vec::new();
        assert!(enter_delve(&mut world, 1, "eastbrook_hollow", &mut events));
        events.clear();

        assert!(!try_advance_delve(&mut world, 1, &mut events));
        assert_eq!(world.get::<InstanceAt>(1).unwrap().delve_room, Some(0));
        assert!(events.is_empty());
    }

    #[test]
    fn world_host_dispatches_enter_and_advance_actions() {
        let mut sim = crate::Sim::new_eastbrook("Delver", PlayerClass::Warrior);
        let player_id = sim.player_id;
        let def = woc_content::delve("eastbrook_hollow").unwrap();
        if let Some(t) = sim.world.get_mut::<Transform>(player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }

        WorldHost::interact(
            &mut sim,
            player_id,
            0,
            InteractAction::EnterDelve {
                delve_id: "eastbrook_hollow".into(),
            },
        );
        assert_eq!(
            sim.world
                .get::<InstanceAt>(player_id)
                .and_then(|i| i.delve_room),
            Some(0)
        );
        assert!(sim.world.next_id() > sim.world.live_ids().max().unwrap_or(0));

        let instance_key = sim
            .world
            .get::<InstanceAt>(player_id)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        defeat_current_room(&mut sim.world, &instance_key);
        WorldHost::interact(&mut sim, player_id, 0, InteractAction::AdvanceDelve);
        assert_eq!(
            sim.world
                .get::<InstanceAt>(player_id)
                .and_then(|i| i.delve_room),
            Some(1)
        );
        assert!(sim.world.next_id() > sim.world.live_ids().max().unwrap_or(0));
    }
}
