//! Dungeon instance enter/leave helpers and boss shell spawning.

use crate::entity::Entity;
use crate::zones::load_overworld_zone;
use woc_content::{dungeon, DungeonDef};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Enter a content-defined dungeon and spawn its boss shell.
pub fn enter_dungeon(
    entities: &mut Vec<Entity>,
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

    let boss_id = entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    entities.retain(|entity| {
        !matches!(
            entity.kind,
            EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
        )
    });

    let instance_zone = format!("instance:{}", def.id);
    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("validated player must survive instance cleanup");
    player.x = def.entrance_x;
    player.z = def.entrance_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.zone_id = instance_zone.clone();
    player.instance_id = Some(def.id.to_string());
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.cast = None;
    player.threat.clear();

    entities.push(create_boss_shell(boss_id, def, &instance_zone));
    events.push(SimEvent::InstanceEntered {
        player: player_id,
        dungeon_id: def.id.to_string(),
    });
    true
}

/// Leave the active instance and rebuild its overworld entrance zone.
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
    let Some(def) = dungeon(&instance_id) else {
        return false;
    };

    if !load_overworld_zone(entities, player_id, def.zone_id) {
        return false;
    }

    events.push(SimEvent::InstanceLeft { player: player_id });
    true
}

fn create_boss_shell(id: EntityId, def: &DungeonDef, zone_id: &str) -> Entity {
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
    boss.zone_id = zone_id.to_string();
    boss.instance_id = Some(def.id.to_string());
    boss
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_mob_from_template, create_player};
    use woc_content::{PlayerClass, EASTBROOK};

    #[test]
    fn enter_spawns_instance_local_boss_and_emits_event() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let overworld_mob =
            create_mob_from_template(2, "young_wolf", 4.0, 4.0).expect("overworld mob");
        let mut entities = vec![player, overworld_mob];
        let mut events = Vec::new();

        assert!(enter_dungeon(
            &mut entities,
            1,
            "eastbrook_crypt",
            &mut events
        ));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.instance_id.as_deref(), Some("eastbrook_crypt"));
        assert_eq!(player.zone_id, "instance:eastbrook_crypt");

        let boss = entities
            .iter()
            .find(|entity| entity.template_id.as_deref() == Some("crypt_warden"))
            .expect("boss shell");
        assert_eq!(boss.kind, EntityKind::Mob);
        assert_eq!(boss.instance_id.as_deref(), Some("eastbrook_crypt"));
        assert_eq!(boss.zone_id, player.zone_id);
        assert!(boss.hp > 0.0);
        assert!(!entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("young_wolf")));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::InstanceEntered { player: 1, dungeon_id }
                if dungeon_id == "eastbrook_crypt"
        )));
    }

    #[test]
    fn leave_returns_to_overworld_spawn_and_removes_boss() {
        let player = create_player(1, "Delver", PlayerClass::Warrior, 2.0, 4.0);
        let mut entities = vec![player];
        let mut events = Vec::new();
        assert!(enter_dungeon(
            &mut entities,
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
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("young_wolf")));
        assert!(events
            .iter()
            .any(|event| matches!(event, SimEvent::InstanceLeft { player: 1 })));
    }
}
