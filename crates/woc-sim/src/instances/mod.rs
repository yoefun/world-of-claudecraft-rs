//! Dungeon instance enter/leave helpers and boss shell spawning.
//!
//! Each enter creates (or joins) a unique instance key so parties do not share
//! bosses and overworld actors are never wiped.

use crate::entity::Entity;
use crate::social::party::PartyRoster;
use crate::zones::load_overworld_zone;
use woc_content::{dungeon, DungeonDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Content dungeon id embedded in `instance_id` (`{dungeon}#{seq}`).
pub fn dungeon_id_from_instance(instance_id: &str) -> &str {
    instance_id.split('#').next().unwrap_or(instance_id)
}

/// Enter a content-defined dungeon and spawn its boss shell.
///
/// Does **not** wipe overworld mobs/NPCs. Spawns an instance-local boss tagged
/// with a unique `instance_id`. Party members already inside the same dungeon
/// share that instance.
pub fn enter_dungeon(
    entities: &mut Vec<Entity>,
    next_id: &mut EntityId,
    parties: &PartyRoster,
    player_id: EntityId,
    dungeon_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = dungeon(dungeon_id) else {
        return false;
    };
    let Some(player) = entities
        .iter()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
    else {
        return false;
    };
    if player.level < def.min_level || player.instance_id.is_some() {
        return false;
    }

    let instance_key = find_party_instance(entities, parties, player_id, dungeon_id)
        .unwrap_or_else(|| {
            let seq = *next_id;
            *next_id = next_id.saturating_add(1);
            format!("{dungeon_id}#{seq}")
        });

    let need_boss = !entities.iter().any(|e| {
        e.kind == EntityKind::Mob
            && e.instance_id.as_deref() == Some(instance_key.as_str())
            && e.template_id.as_deref() == Some(def.boss_id)
            && e.alive
    });

    if need_boss {
        let boss_id = *next_id;
        *next_id = next_id.saturating_add(1);
        entities.push(create_boss_shell(boss_id, def, &instance_key));
    }

    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("validated player must survive instance enter");
    player.x = def.entrance_x;
    player.z = def.entrance_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.zone_id = format!("instance:{}", dungeon_id_from_instance(&instance_key));
    player.instance_id = Some(instance_key);
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.cast = None;
    player.threat.clear();

    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    true
}

fn find_party_instance(
    entities: &[Entity],
    parties: &PartyRoster,
    player_id: EntityId,
    dungeon_id: &str,
) -> Option<String> {
    let members = parties.members_of(player_id)?;
    for mid in members {
        if mid == player_id {
            continue;
        }
        if let Some(inst) = entities
            .iter()
            .find(|e| e.id == mid && e.kind == EntityKind::Player)
            .and_then(|e| e.instance_id.clone())
        {
            if dungeon_id_from_instance(&inst) == dungeon_id {
                return Some(inst);
            }
        }
    }
    None
}

/// Leave the active instance; only despawn boss if no players remain inside.
pub fn leave_instance(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(instance_id) = entities
        .iter()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
        .and_then(|player| player.instance_id.clone())
    else {
        return false;
    };
    let dungeon_id = dungeon_id_from_instance(&instance_id);
    let Some(def) = dungeon(dungeon_id) else {
        return false;
    };

    if !load_overworld_zone(entities, player_id, def.zone_id) {
        return false;
    }

    let others_inside = entities.iter().any(|e| {
        e.kind == EntityKind::Player && e.instance_id.as_deref() == Some(instance_id.as_str())
    });
    if !others_inside {
        entities.retain(|entity| {
            entity.kind == EntityKind::Player
                || entity.instance_id.as_deref() != Some(instance_id.as_str())
        });
    }

    events.push(SimEvent::InstanceLeft { player: player_id });
    true
}

fn create_boss_shell(id: EntityId, def: &DungeonDef, instance_key: &str) -> Entity {
    let mut boss = Entity::blank(
        id,
        EntityKind::Mob,
        def.boss_name,
        Some(def.boss_id),
        def.boss_x,
        def.boss_z,
    );
    boss.level = def.boss_level;
    boss.hp = def.boss_hp;
    boss.hp_max = def.boss_hp;
    boss.attack_damage = def.boss_attack_damage;
    boss.xp_value = def.boss_level.saturating_mul(50);
    boss.zone_id = format!("instance:{}", dungeon_id_from_instance(instance_key));
    boss.instance_id = Some(instance_key.to_string());
    boss
}

/// Whether two entities share interaction space (same instance or both overworld).
pub fn same_instance_space(a: &Entity, b: &Entity) -> bool {
    match (&a.instance_id, &b.instance_id) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player};
    use crate::social::party::PartyRoster;
    use woc_content::{PlayerClass, EASTBROOK};

    #[test]
    fn enter_preserves_overworld_and_uses_unique_instance() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let overworld_mob =
            create_mob_from_template(2, "young_wolf", 4.0, 4.0).expect("overworld mob");
        let mut entities = vec![player, overworld_mob];
        let mut next_id = 3;
        let parties = PartyRoster::new();
        let mut events = Vec::new();

        assert!(enter_dungeon(
            &mut entities,
            &mut next_id,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert!(player
            .instance_id
            .as_deref()
            .unwrap()
            .starts_with("eastbrook_crypt#"));
        assert_eq!(player.zone_id, "instance:eastbrook_crypt");

        let boss = entities
            .iter()
            .find(|entity| entity.template_id.as_deref() == Some("crypt_warden"))
            .expect("boss shell");
        assert_eq!(boss.instance_id, player.instance_id);
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("young_wolf")));
    }

    #[test]
    fn party_members_share_instance() {
        let mut entities = vec![
            create_player(1, "A", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "B", PlayerClass::Mage, 1.0, 0.0),
        ];
        let mut parties = PartyRoster::new();
        let world = crate::ecs::spawn::world_from_entities(&entities);
        let _ = parties.invite(1, "B", &world);
        let _ = parties.accept(2, &world);
        let mut next_id = 3;
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut entities,
            &mut next_id,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        let key = entities[0].instance_id.clone().unwrap();
        assert!(enter_dungeon(
            &mut entities,
            &mut next_id,
            &parties,
            2,
            "eastbrook_crypt",
            &mut events
        ));
        assert_eq!(entities[1].instance_id.as_deref(), Some(key.as_str()));
        assert_eq!(
            entities
                .iter()
                .filter(|e| e.template_id.as_deref() == Some("crypt_warden"))
                .count(),
            1
        );
    }

    #[test]
    fn leave_returns_to_overworld_spawn_and_removes_boss() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let mut entities = vec![player];
        let mut next_id = 2;
        let parties = PartyRoster::new();
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut entities,
            &mut next_id,
            &parties,
            1,
            "eastbrook_crypt",
            &mut events
        ));
        events.clear();

        assert!(leave_instance(&mut entities, 1, &mut events));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.instance_id, None);
        assert_eq!(player.zone_id, "eastbrook");
        assert_eq!(player.x, EASTBROOK.player_spawn_x);
        assert_eq!(player.z, EASTBROOK.player_spawn_z);
        assert!(!entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("crypt_warden")));
        assert!(events
            .iter()
            .any(|event| matches!(event, SimEvent::InstanceLeft { player: 1 })));
    }
}
