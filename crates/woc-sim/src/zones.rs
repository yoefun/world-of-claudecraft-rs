//! Overworld zone transitions and content-table population.

use crate::entity::{create_mob_from_template, create_npc_from_template, Entity};
use woc_content::{ZoneLayout, EASTBROOK, EASTFEN};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Resolve a supported overworld zone to its spawn layout.
pub fn zone_layout(zone_id: &str) -> Option<&'static ZoneLayout> {
    match zone_id {
        "eastbrook" => Some(&EASTBROOK),
        "eastfen" => Some(&EASTFEN),
        _ => None,
    }
}

/// Move a player through a portal and replace local world actors.
///
/// Only transient world entities are rebuilt. Player entities (including their
/// inventory, progression, talents, and equipment) remain in place.
pub fn enter_portal(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    zone_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    if !load_overworld_zone(entities, player_id, zone_id) {
        return false;
    }

    events.push(SimEvent::ZoneChanged {
        player: player_id,
        zone_id: zone_id.to_string(),
    });
    true
}

/// Teleport `player_id` and repopulate a zone without emitting an event.
///
/// This is shared with instance exits, which emit `InstanceLeft` instead.
pub(crate) fn load_overworld_zone(
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    zone_id: &str,
) -> bool {
    let Some(layout) = zone_layout(zone_id) else {
        return false;
    };
    if !entities
        .iter()
        .any(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
    {
        return false;
    }

    let mut next_id = entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    // Loot belongs to the world population being unloaded too.
    entities.retain(|entity| {
        !matches!(
            entity.kind,
            EntityKind::Mob | EntityKind::Npc | EntityKind::Loot
        )
    });

    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("validated player must survive zone cleanup");
    player.x = layout.player_spawn_x;
    player.z = layout.player_spawn_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.zone_id = zone_id.to_string();
    player.instance_id = None;
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.cast = None;
    player.threat.clear();

    for spot in layout.npcs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut npc) = create_npc_from_template(id, spot.npc_id, spot.x, spot.z) {
            npc.zone_id = zone_id.to_string();
            entities.push(npc);
        }
    }

    for spot in layout.mobs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut mob) = create_mob_from_template(id, spot.mob_id, spot.x, spot.z) {
            mob.zone_id = zone_id.to_string();
            entities.push(mob);
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{
        count_item, create_mob_from_template, create_npc_from_template, create_player, grant_into,
    };
    use woc_content::PlayerClass;

    #[test]
    fn eastbrook_to_eastfen_preserves_player_progression() {
        let mut player = create_player(1, "Traveler", PlayerClass::Mage, 30.0, 30.0);
        player.xp = 73;
        player.level = 2;
        player.talent_points = 2;
        player.talents.insert("arcane_focus".into(), 1);
        assert!(grant_into(&mut player.inventory, "wolf_fang", 2));

        let old_mob = create_mob_from_template(2, "young_wolf", 0.0, 0.0).expect("eastbrook mob");
        let old_npc =
            create_npc_from_template(3, "captain_alden", 0.0, 0.0).expect("eastbrook npc");
        let mut entities = vec![player, old_mob, old_npc];
        let mut events = Vec::new();

        assert!(enter_portal(&mut entities, 1, "eastfen", &mut events));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.zone_id, "eastfen");
        assert_eq!(player.xp, 73);
        assert_eq!(player.level, 2);
        assert_eq!(player.talent_points, 2);
        assert_eq!(player.talents.get("arcane_focus"), Some(&1));
        assert_eq!(count_item(&player.inventory, "wolf_fang"), 2);
        assert_eq!(player.x, EASTFEN.player_spawn_x);
        assert_eq!(player.z, EASTFEN.player_spawn_z);

        assert!(!entities.iter().any(|entity| {
            matches!(
                entity.template_id.as_deref(),
                Some("young_wolf" | "captain_alden")
            )
        }));
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("fen_crawler")));
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("warden_selene")));
        assert!(entities
            .iter()
            .filter(|entity| matches!(entity.kind, EntityKind::Mob | EntityKind::Npc))
            .all(|entity| entity.zone_id == "eastfen"));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::ZoneChanged { player: 1, zone_id } if zone_id == "eastfen"
        )));
    }

    #[test]
    fn unsupported_zone_does_not_mutate_player() {
        let player = create_player(1, "Traveler", PlayerClass::Warrior, 9.0, 11.0);
        let mut entities = vec![player];
        let mut events = Vec::new();

        assert!(!enter_portal(&mut entities, 1, "missing_zone", &mut events));
        assert_eq!((entities[0].x, entities[0].z), (9.0, 11.0));
        assert!(events.is_empty());
    }
}
