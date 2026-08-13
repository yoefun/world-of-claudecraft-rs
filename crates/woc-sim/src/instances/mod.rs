//! Dungeon instance enter/leave helpers and boss shell spawning.
//!
//! Each enter creates (or joins) a unique instance key so parties do not share
//! bosses and overworld actors are never wiped.

use crate::ecs::components::{
    Bags, Combat, Health, Home, Identity, InstanceAt, LootTable, Respawn, Threat, Transform,
};
use crate::ecs::World;
use crate::social::party::PartyRoster;
use crate::zones::load_overworld_zone;
use woc_content::{dungeon, DungeonDef, DungeonTrashSpot};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Content dungeon id embedded in `instance_id` (`{dungeon}#{seq}`).
pub fn dungeon_id_from_instance(instance_id: &str) -> &str {
    instance_id.split('#').next().unwrap_or(instance_id)
}

/// Enter a content-defined dungeon and spawn its boss shell.
pub fn enter_dungeon(
    world: &mut World,
    parties: &PartyRoster,
    player_id: EntityId,
    dungeon_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = dungeon(dungeon_id) else {
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
    if level < def.min_level || in_instance {
        return false;
    }

    let instance_key =
        find_party_instance(world, parties, player_id, dungeon_id).unwrap_or_else(|| {
            let seq = world.next_id();
            world.set_next_id(seq.saturating_add(1));
            format!("{dungeon_id}#{seq}")
        });

    let need_boss = !world.ids::<Identity>().into_iter().any(|id| {
        world.get::<Identity>(id).is_some_and(|identity| {
            identity.kind == EntityKind::Mob
                && world
                    .get::<InstanceAt>(id)
                    .and_then(|i| i.instance_id.as_deref())
                    == Some(instance_key.as_str())
                && identity.template_id.as_deref() == Some(def.boss_id)
                && world.get::<Health>(id).map(|h| h.alive).unwrap_or(false)
        })
    });

    if need_boss {
        let boss_id = world.next_id();
        spawn_boss_shell(world, boss_id, def, &instance_key);
    }
    if !instance_has_living_trash(world, &instance_key, def) {
        spawn_trash_packs(world, def, &instance_key);
    }

    let spawn_y = crate::ecs::spawn::ground_at(def.entrance_x, def.entrance_z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = def.entrance_x;
        t.z = def.entrance_z;
        t.y = spawn_y;
    }
    if let Some(identity) = world.get_mut::<Identity>(player_id) {
        identity.zone_id = format!("instance:{}", dungeon_id_from_instance(&instance_key));
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.instance_id = Some(instance_key);
        inst.delve_room = None;
    }
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

    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    true
}

fn find_party_instance(
    world: &World,
    parties: &PartyRoster,
    player_id: EntityId,
    dungeon_id: &str,
) -> Option<String> {
    let members = parties.members_of(player_id)?;
    for mid in members {
        if mid == player_id {
            continue;
        }
        if let Some(inst) = world
            .get::<InstanceAt>(mid)
            .and_then(|i| i.instance_id.clone())
        {
            if dungeon_id_from_instance(&inst) == dungeon_id {
                return Some(inst);
            }
        }
    }
    None
}

/// Leave the active instance; only despawn boss if no players remain inside.
pub fn leave_instance(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    let Some(instance_id) = world
        .get::<InstanceAt>(player_id)
        .and_then(|i| i.instance_id.clone())
    else {
        return false;
    };
    let dungeon_id = dungeon_id_from_instance(&instance_id);
    let Some(def) = dungeon(dungeon_id) else {
        return false;
    };

    if !load_overworld_zone(world, player_id, def.zone_id) {
        return false;
    }

    let others_inside = world.ids::<Identity>().into_iter().any(|id| {
        world.get::<Identity>(id).is_some_and(|identity| {
            identity.kind == EntityKind::Player
                && world
                    .get::<InstanceAt>(id)
                    .and_then(|i| i.instance_id.as_deref())
                    == Some(instance_id.as_str())
        })
    });
    if !others_inside {
        let to_despawn: Vec<EntityId> = world
            .live_ids()
            .filter(|&id| {
                world.get::<Identity>(id).is_some_and(|identity| {
                    identity.kind != EntityKind::Player
                        && world
                            .get::<InstanceAt>(id)
                            .and_then(|i| i.instance_id.as_deref())
                            == Some(instance_id.as_str())
                })
            })
            .collect();
        for id in to_despawn {
            world.despawn(id);
        }
    }

    events.push(SimEvent::InstanceLeft { player: player_id });
    true
}

fn spawn_boss_shell(world: &mut World, id: EntityId, def: &DungeonDef, instance_key: &str) {
    crate::ecs::spawn::adopt_fresh_id(world, id);
    world.insert(
        id,
        Identity {
            kind: EntityKind::Mob,
            name: def.boss_name.to_string(),
            template_id: Some(def.boss_id.to_string()),
            zone_id: format!("instance:{}", dungeon_id_from_instance(instance_key)),
        },
    );
    let y = crate::ecs::spawn::ground_at(def.boss_x, def.boss_z);
    world.insert(
        id,
        Transform {
            x: def.boss_x,
            y,
            z: def.boss_z,
            yaw: 0.0,
        },
    );
    world.insert(
        id,
        Health {
            hp: def.boss_hp,
            hp_max: def.boss_hp,
            alive: true,
            level: def.boss_level,
        },
    );
    world.insert(
        id,
        Combat {
            attack_damage: def.boss_attack_damage,
            armor: 0.0,
            swing_timer: 0.0,
            ability_cd: 0.0,
            auto_attack: false,
            target: None,
            gcd: 0.0,
            cast: None,
            cast_lockout: 0.0,
        },
    );
    world.insert(id, crate::ecs::components::Auras { auras: Vec::new() });
    world.insert(
        id,
        Home {
            home_x: def.boss_x,
            home_z: def.boss_z,
        },
    );
    world.insert(id, Threat::default());
    world.insert(
        id,
        LootTable {
            loot_copper: 0,
            loot_item: None,
            xp_value: def.boss_level.saturating_mul(50),
        },
    );
    world.insert(id, Respawn::default());
    world.insert(
        id,
        InstanceAt {
            instance_id: Some(instance_key.to_string()),
            delve_room: None,
        },
    );
}

fn instance_has_living_trash(world: &World, instance_key: &str, def: &DungeonDef) -> bool {
    world.ids::<LootTable>().into_iter().any(|id| {
        world.get::<Health>(id).is_some_and(|h| h.alive)
            && world
                .get::<InstanceAt>(id)
                .and_then(|i| i.instance_id.as_deref())
                == Some(instance_key)
            && world
                .get::<Identity>(id)
                .and_then(|identity| identity.template_id.as_deref())
                .is_some_and(|tid| def.trash.iter().any(|spot| spot.mob_id == tid))
    })
}

fn spawn_trash_packs(world: &mut World, def: &DungeonDef, instance_key: &str) {
    let zone_id = format!("instance:{}", dungeon_id_from_instance(instance_key));
    for spot in def.trash {
        spawn_trash_spot(world, spot, instance_key, &zone_id);
    }
}

fn spawn_trash_spot(world: &mut World, spot: &DungeonTrashSpot, instance_key: &str, zone_id: &str) {
    for i in 0..spot.count {
        let id = world.next_id();
        let x = spot.x + i as f32 * 1.2;
        let Some(spawned) =
            crate::ecs::spawn::create_mob_from_template(world, id, spot.mob_id, x, spot.z)
        else {
            continue;
        };
        if let Some(identity) = world.get_mut::<Identity>(spawned) {
            identity.zone_id = zone_id.to_string();
        }
        if let Some(inst) = world.get_mut::<InstanceAt>(spawned) {
            inst.instance_id = Some(instance_key.to_string());
        }
    }
}

/// Whether two entities share interaction space (same instance or both overworld).
pub fn same_instance_space(world: &World, a: EntityId, b: EntityId) -> bool {
    let a_inst = world
        .get::<InstanceAt>(a)
        .and_then(|i| i.instance_id.as_deref());
    let b_inst = world
        .get::<InstanceAt>(b)
        .and_then(|i| i.instance_id.as_deref());
    match (a_inst, b_inst) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_content::{PlayerClass, EASTBROOK, MIREFEN};

    #[test]
    fn enter_preserves_overworld_and_uses_unique_instance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 4.0, 4.0)
            .expect("overworld mob");
        let parties = PartyRoster::new();
        let mut events = Vec::new();

        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));

        let player_inst = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        assert!(player_inst.starts_with("eastbrook_crypt#"));
        assert_eq!(
            world.get::<Identity>(1).unwrap().zone_id,
            "instance:eastbrook_crypt"
        );

        let boss_id = world
            .ids::<Identity>()
            .into_iter()
            .find(|&id| {
                world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some("crypt_warden")
            })
            .expect("boss shell");
        assert_eq!(
            world
                .get::<InstanceAt>(boss_id)
                .unwrap()
                .instance_id
                .as_deref(),
            Some(player_inst.as_str())
        );
        assert!(world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some("young_wolf")
        }));
    }

    #[test]
    fn party_members_share_instance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "A", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "B", PlayerClass::Mage, 1.0, 0.0);
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "B", &world);
        let _ = parties.accept(2, &world);
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        let key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            2,
            "eastbrook_crypt",
            &mut events
        ));
        assert_eq!(
            world
                .get::<InstanceAt>(2)
                .and_then(|i| i.instance_id.clone()),
            Some(key)
        );
        assert_eq!(
            world
                .ids::<Identity>()
                .into_iter()
                .filter(|&id| {
                    world
                        .get::<Identity>(id)
                        .and_then(|i| i.template_id.as_deref())
                        == Some("crypt_warden")
                })
                .count(),
            1
        );
    }

    #[test]
    fn leave_returns_to_overworld_spawn_and_removes_boss() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        events.clear();

        assert!(leave_instance(&mut world, 1, &mut events));

        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "eastbrook");
        let t = world.get::<Transform>(1).unwrap();
        assert_eq!(t.x, EASTBROOK.player_spawn_x);
        assert_eq!(t.z, EASTBROOK.player_spawn_z);
        assert!(!world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some("crypt_warden")
        }));
        assert!(events
            .iter()
            .any(|event| matches!(event, SimEvent::InstanceLeft { player: 1 })));
    }

    fn living_instance_loot<'a>(
        world: &'a World,
        instance_key: &str,
        template_id: &str,
    ) -> Vec<EntityId> {
        world
            .ids::<LootTable>()
            .into_iter()
            .filter(|&id| {
                world.get::<Health>(id).is_some_and(|h| h.alive)
                    && world
                        .get::<InstanceAt>(id)
                        .and_then(|i| i.instance_id.as_deref())
                        == Some(instance_key)
                    && world
                        .get::<Identity>(id)
                        .and_then(|identity| identity.template_id.as_deref())
                        == Some(template_id)
            })
            .collect()
    }

    #[test]
    fn enter_crypt_spawns_trash_and_party_credit_still_works() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "A", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "B", PlayerClass::Mage, 1.0, 0.0);
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "B", &world);
        let _ = parties.accept(2, &world);
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        assert!(enter_dungeon(
            &mut world,
            &parties,
            2,
            "eastbrook_crypt",
            &mut events
        ));
        let key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        let wolves = living_instance_loot(&world, &key, "young_wolf");
        let boars = living_instance_loot(&world, &key, "young_boar");
        let bosses = living_instance_loot(&world, &key, "crypt_warden");
        assert!(
            wolves.len() + boars.len() >= 2,
            "crypt needs ≥2 living trash, got wolves={} boars={}",
            wolves.len(),
            boars.len()
        );
        assert_eq!(bosses.len(), 1);
        assert_eq!(
            living_instance_loot(&world, &key, "young_wolf").len()
                + living_instance_loot(&world, &key, "young_boar").len(),
            wolves.len() + boars.len(),
            "joining a party must not duplicate trash packs"
        );
        let mates = crate::social::party::kill_credit_share(&parties, &world, 1);
        assert!(mates.contains(&2), "party kill credit must still share");
    }

    #[test]
    fn enter_mirefen_barrow_and_leave_returns_to_mirefen() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 3.0, 308.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "mirefen_barrow",
            &mut events
        ));
        let key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        assert!(key.starts_with("mirefen_barrow#"));
        assert_eq!(
            world.get::<Identity>(1).unwrap().zone_id,
            "instance:mirefen_barrow"
        );
        assert_eq!(living_instance_loot(&world, &key, "barrow_hag").len(), 1);
        assert!(
            living_instance_loot(&world, &key, "fen_crawler").len()
                + living_instance_loot(&world, &key, "mire_toad").len()
                >= 2
        );
        events.clear();
        assert!(leave_instance(&mut world, 1, &mut events));
        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "mirefen");
        let t = world.get::<Transform>(1).unwrap();
        assert_eq!(t.x, MIREFEN.player_spawn_x);
        assert_eq!(t.z, MIREFEN.player_spawn_z);
        assert!(living_instance_loot(&world, &key, "barrow_hag").is_empty());
    }
}
