//! Dungeon instance enter/leave helpers and boss shell spawning.
//!
//! Each enter creates (or joins) a unique instance key so parties do not share
//! bosses and overworld actors are never wiped.

use crate::ecs::components::{
    Bags, Combat, Health, Home, Identity, InstanceAt, LootTable, Respawn, Threat, Transform,
};
use crate::ecs::World;
use crate::social::party::PartyRoster;
use woc_content::{delve, dungeon, DungeonDef, DungeonTrashSpot};
use woc_protocol::{EntityId, EntityKind, SimEvent};

pub const INSTANCE_ENTER_RANGE: f32 = 5.0;

/// Content dungeon id embedded in `instance_id` (`{dungeon}#{seq}`).
pub fn dungeon_id_from_instance(instance_id: &str) -> &str {
    instance_id.split('#').next().unwrap_or(instance_id)
}

pub fn parent_zone_for_instance_key(instance_id: &str) -> Option<&'static str> {
    let content_id = dungeon_id_from_instance(instance_id);
    dungeon(content_id)
        .map(|d| d.zone_id)
        .or_else(|| delve(content_id).map(|d| d.zone_id))
}

pub fn follow_owner_into_instance(world: &mut World, player_id: EntityId) {
    let Some(pet) = crate::pet::find_pet(world, player_id) else {
        return;
    };
    let inst = world
        .get::<InstanceAt>(player_id)
        .cloned()
        .unwrap_or_default();
    let zone = world
        .get::<Identity>(player_id)
        .map(|i| i.zone_id.clone())
        .unwrap_or_default();
    let (px, pz) = world
        .get::<Transform>(player_id)
        .map(|t| (t.x, t.z))
        .unwrap_or((0.0, 0.0));
    if world.get::<InstanceAt>(pet).is_none() {
        world.insert(pet, InstanceAt::default());
    }
    if let Some(slot) = world.get_mut::<InstanceAt>(pet) {
        *slot = inst;
    }
    if let Some(identity) = world.get_mut::<Identity>(pet) {
        identity.zone_id = zone;
    }
    if let Some(t) = world.get_mut::<Transform>(pet) {
        t.x = px + 1.5;
        t.z = pz;
        t.y = crate::ecs::spawn::ground_at(t.x, t.z);
    }
}

fn xz_dist(world: &World, player_id: EntityId, x: f32, z: f32) -> f32 {
    world
        .get::<Transform>(player_id)
        .map(|t| {
            let dx = t.x - x;
            let dz = t.z - z;
            (dx * dx + dz * dz).sqrt()
        })
        .unwrap_or(f32::MAX)
}

/// Enter a content-defined dungeon and spawn its boss shell.
pub fn enter_dungeon(
    world: &mut World,
    parties: &PartyRoster,
    player_id: EntityId,
    dungeon_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    if world.get::<Identity>(player_id).map(|i| i.kind) != Some(EntityKind::Player) {
        return false;
    }
    let Some((alive, level)) = world
        .get::<Health>(player_id)
        .map(|health| (health.alive, health.level))
    else {
        return false;
    };
    if !alive {
        return false;
    }
    let Some(def) = dungeon(dungeon_id) else {
        events.push(SimEvent::Toast {
            message: "There is no such instance.".into(),
        });
        return false;
    };
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
    if xz_dist(world, player_id, def.entrance_x, def.entrance_z) > INSTANCE_ENTER_RANGE {
        events.push(SimEvent::Toast {
            message: "You must be closer to the entrance.".into(),
        });
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
    follow_owner_into_instance(world, player_id);
    crate::mount::dismount(world, player_id, events);
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

    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Entered {}.", def.name),
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

pub(crate) fn despawn_instance_if_empty(world: &mut World, instance_id: &str) {
    let others_inside = world.ids::<Identity>().into_iter().any(|id| {
        world.get::<Identity>(id).is_some_and(|identity| {
            identity.kind == EntityKind::Player
                && world
                    .get::<InstanceAt>(id)
                    .and_then(|i| i.instance_id.as_deref())
                    == Some(instance_id)
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
                            == Some(instance_id)
                })
            })
            .collect();
        for id in to_despawn {
            world.despawn(id);
        }
    }
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
    if let Some(def) = dungeon(dungeon_id) {
        if !crate::zones::load_overworld_zone_at(
            world,
            player_id,
            def.zone_id,
            def.entrance_x,
            def.entrance_z,
        ) {
            return false;
        }
    } else if let Some(def) = woc_content::delve(dungeon_id) {
        if !crate::zones::load_overworld_zone_at(
            world,
            player_id,
            def.zone_id,
            def.entrance_x,
            def.entrance_z,
        ) {
            return false;
        }
    } else {
        return false;
    }

    despawn_instance_if_empty(world, &instance_id);

    events.push(SimEvent::InstanceLeft { player: player_id });
    events.push(SimEvent::Toast {
        message: "Left the instance.".into(),
    });
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
            spell_power: 0.0,
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
    use woc_content::PlayerClass;

    fn place_at_dungeon(world: &mut World, player_id: EntityId, dungeon_id: &str) {
        let def = woc_content::dungeon(dungeon_id).unwrap();
        if let Some(t) = world.get_mut::<Transform>(player_id) {
            t.x = def.entrance_x;
            t.z = def.entrance_z;
        }
        if let Some(h) = world.get_mut::<Health>(player_id) {
            h.level = def.min_level.max(h.level);
        }
    }

    #[test]
    fn enter_rejects_when_too_far() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Far", PlayerClass::Warrior, 2.0, 4.0);
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(!enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You must be closer to the entrance."
        )));
        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
    }

    #[test]
    fn final_review_dead_player_cannot_enter_dungeon_silently() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Dead", PlayerClass::Warrior, 0.0, 0.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        if let Some(health) = world.get_mut::<Health>(1) {
            health.alive = false;
            health.hp = 0.0;
        }
        let before_next_id = world.next_id();
        let before_live = world.live_ids().count();
        let mut events = Vec::new();

        assert!(!enter_dungeon(
            &mut world,
            &PartyRoster::new(),
            1,
            "eastbrook_crypt",
            &mut events,
        ));
        assert!(events.is_empty());
        assert_eq!(world.next_id(), before_next_id);
        assert_eq!(world.live_ids().count(), before_live);
        assert!(world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.as_ref())
            .is_none());
    }

    #[test]
    fn enter_rejects_low_level_at_barrow_entrance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Low", PlayerClass::Warrior, 25.0, 430.0);
        place_at_dungeon(&mut world, 1, "mirefen_barrow");
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 1;
        }
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(!enter_dungeon(
            &mut world,
            &parties,
            1,
            "mirefen_barrow",
            &mut events
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message }
                if message == "You must be level 3 to enter Mirefen Barrow."
        )));
    }

    #[test]
    fn leave_lands_on_crypt_entrance_not_zone_spawn() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
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
        let t = world.get::<Transform>(1).unwrap();
        let def = woc_content::dungeon("eastbrook_crypt").unwrap();
        assert!((t.x - def.entrance_x).abs() < 1e-3);
        assert!((t.z - def.entrance_z).abs() < 1e-3);
        assert_eq!(world.get::<Identity>(1).unwrap().zone_id, "eastbrook");
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Left the instance."
        )));
    }

    #[test]
    fn hunter_pet_follows_into_crypt() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hunt", PlayerClass::Hunter, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        let mut events = Vec::new();
        assert!(crate::pet::summon_pet(&mut world, 1, &mut events));
        let pet = crate::pet::find_pet(&world, 1).unwrap();
        let parties = PartyRoster::new();
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
        assert_eq!(
            world
                .get::<InstanceAt>(pet)
                .and_then(|i| i.instance_id.clone())
                .as_deref(),
            Some(key.as_str())
        );
    }

    #[test]
    fn enter_dungeon_dismounts_active_mount() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Rider", PlayerClass::Warrior, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        world
            .get_mut::<crate::ecs::components::Health>(1)
            .unwrap()
            .level = 10;
        world
            .get_mut::<crate::ecs::components::Riding>(1)
            .unwrap()
            .rank = 1;
        world
            .get_mut::<crate::ecs::components::Riding>(1)
            .unwrap()
            .known
            .insert("brown_pony".into());
        let mut events = Vec::new();
        assert!(crate::mount::summon_mount(
            &mut world,
            1,
            "brown_pony",
            &mut events
        ));
        assert!(world
            .get::<crate::ecs::components::Riding>(1)
            .unwrap()
            .active_id
            .is_some());

        let parties = PartyRoster::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        assert!(world
            .get::<crate::ecs::components::Riding>(1)
            .unwrap()
            .active_id
            .is_none());
    }

    #[test]
    fn enter_preserves_overworld_and_uses_unique_instance() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
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
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        place_at_dungeon(&mut world, 2, "eastbrook_crypt");
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "B", &world, 0);
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
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
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
        let def = woc_content::dungeon("eastbrook_crypt").unwrap();
        assert_eq!(t.x, def.entrance_x);
        assert_eq!(t.z, def.entrance_z);
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

    fn instance_non_players(world: &World, instance_key: &str) -> usize {
        world
            .live_ids()
            .filter(|&id| {
                world.get::<Identity>(id).is_some_and(|identity| {
                    identity.kind != EntityKind::Player
                        && world
                            .get::<InstanceAt>(id)
                            .and_then(|i| i.instance_id.as_deref())
                            == Some(instance_key)
                })
            })
            .count()
    }

    #[test]
    fn final_review_hearth_keeps_occupied_instance_and_cleans_last_exit_silently() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "A", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "B", PlayerClass::Mage, 0.0, 0.0);
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        place_at_dungeon(&mut world, 2, "eastbrook_crypt");
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "B", &world, 0);
        let _ = parties.accept(2, &world);
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut world,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events,
        ));
        assert!(enter_dungeon(
            &mut world,
            &parties,
            2,
            "eastbrook_crypt",
            &mut events,
        ));
        let key = world
            .get::<InstanceAt>(1)
            .and_then(|i| i.instance_id.clone())
            .unwrap();
        let loot_id = world.next_id();
        crate::ecs::spawn::create_loot(&mut world, loot_id, 0.0, 0.0, 5, None);
        world
            .get_mut::<InstanceAt>(loot_id)
            .unwrap()
            .instance_id = Some(key.clone());
        let populated = instance_non_players(&world, &key);
        assert!(populated > 0);

        events.clear();
        assert!(crate::zones::use_hearthstone(&mut world, 1, 0, &mut events));
        assert_eq!(instance_non_players(&world, &key), populated);
        assert!(world.contains(loot_id));
        assert!(!events.iter().any(|event| matches!(
            event,
            SimEvent::Toast { message } if message == "Left the instance."
        )));

        events.clear();
        assert!(crate::zones::use_hearthstone(&mut world, 2, 0, &mut events));
        assert_eq!(instance_non_players(&world, &key), 0);
        assert!(!world.contains(loot_id));
        assert!(!events.iter().any(|event| matches!(
            event,
            SimEvent::Toast { message } if message == "Left the instance."
        )));
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
        place_at_dungeon(&mut world, 1, "eastbrook_crypt");
        place_at_dungeon(&mut world, 2, "eastbrook_crypt");
        let mut parties = PartyRoster::new();
        let _ = parties.invite(1, "B", &world, 0);
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
        place_at_dungeon(&mut world, 1, "mirefen_barrow");
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
        let def = woc_content::dungeon("mirefen_barrow").unwrap();
        assert_eq!(t.x, def.entrance_x);
        assert_eq!(t.z, def.entrance_z);
        assert!(living_instance_loot(&world, &key, "barrow_hag").is_empty());
    }
}
