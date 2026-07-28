//! Overworld zone transitions on the continuous strip.

use crate::entity::{create_mob_from_template, create_npc_from_template, Entity};
use woc_content::{ZoneLayout, EASTBROOK, EASTFEN, MIREFEN, THORNPEAK};
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Resolve a supported overworld zone to its spawn layout.
pub fn zone_layout(zone_id: &str) -> Option<&'static ZoneLayout> {
    match zone_id {
        "eastbrook" | "eastbrook_vale" => Some(&EASTBROOK),
        "eastfen" | "fenbridge" | "mirefen_marsh" => Some(&EASTFEN),
        "mirefen" => Some(&MIREFEN),
        "thornpeak" | "thornpeak_heights" | "highwatch" => Some(&THORNPEAK),
        _ => None,
    }
}

fn layout_zone_tag(zone_id: &str) -> &'static str {
    match zone_id {
        "eastbrook" | "eastbrook_vale" => "eastbrook",
        "eastfen" | "fenbridge" | "mirefen_marsh" => "eastfen",
        "mirefen" => "mirefen",
        "thornpeak" | "thornpeak_heights" | "highwatch" => "thornpeak",
        _ => "unknown",
    }
}

/// Teleport through a portal without wiping other-zone actors.
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
        zone_id: layout_zone_tag(zone_id).to_string(),
    });
    true
}

/// Ensure the destination zone population exists, then teleport the player.
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

    let tag = layout_zone_tag(zone_id);
    ensure_zone_population(entities, layout, tag);

    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id)
        .expect("validated player must survive zone load");
    player.x = layout.player_spawn_x;
    player.z = layout.player_spawn_z;
    player.y = Entity::ground_at(player.x, player.z);
    player.zone_id = tag.to_string();
    player.instance_id = None;
    player.target = None;
    player.auto_attack = false;
    player.open_vendor_npc = None;
    player.cast = None;
    player.threat.clear();
    true
}

fn ensure_zone_population(entities: &mut Vec<Entity>, layout: &ZoneLayout, tag: &str) {
    let has_zone_npc = entities
        .iter()
        .any(|e| e.kind == EntityKind::Npc && e.zone_id == tag && e.template_id.is_some());
    if has_zone_npc {
        return;
    }
    let mut next_id = entities
        .iter()
        .map(|entity| entity.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for spot in layout.npcs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut npc) = create_npc_from_template(id, spot.npc_id, spot.x, spot.z) {
            npc.zone_id = tag.to_string();
            entities.push(npc);
        }
    }
    for spot in layout.mobs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut mob) = create_mob_from_template(id, spot.mob_id, spot.x, spot.z) {
            mob.zone_id = tag.to_string();
            entities.push(mob);
        }
    }
}

/// Populate all overworld layouts into one continuous realm.
pub fn populate_all_overworld(
    entities: &mut Vec<Entity>,
    next_id: &mut EntityId,
    rng: &mut crate::rng::Rng,
) {
    for (layout, tag) in [
        (&EASTBROOK, "eastbrook"),
        (&EASTFEN, "eastfen"),
        (&MIREFEN, "mirefen"),
        (&THORNPEAK, "thornpeak"),
    ] {
        for spot in layout.npcs {
            let id = *next_id;
            *next_id = next_id.saturating_add(1);
            if let Some(mut npc) = create_npc_from_template(id, spot.npc_id, spot.x, spot.z) {
                npc.zone_id = tag.to_string();
                entities.push(npc);
            }
        }
        for spot in layout.mobs {
            let id = *next_id;
            *next_id = next_id.saturating_add(1);
            if let Some(mut mob) = create_mob_from_template(id, spot.mob_id, spot.x, spot.z) {
                mob.x += (rng.next_f32() - 0.5) * 1.5;
                mob.z += (rng.next_f32() - 0.5) * 1.5;
                mob.home_x = mob.x;
                mob.home_z = mob.z;
                mob.y = Entity::ground_at(mob.x, mob.z);
                mob.zone_id = tag.to_string();
                entities.push(mob);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{count_item, create_player, grant_into};
    use woc_content::PlayerClass;

    #[test]
    fn eastbrook_to_eastfen_preserves_player_progression() {
        let mut player = create_player(1, "Traveler", PlayerClass::Mage, 30.0, 30.0);
        player.xp = 73;
        player.level = 2;
        player.talent_points = 2;
        player.talents.insert("arcane_focus".into(), 1);
        assert!(grant_into(&mut player.inventory, "wolf_fang", 2));

        let mut entities = vec![player];
        let mut next_id = 2;
        let mut rng = crate::rng::Rng::new(1);
        populate_all_overworld(&mut entities, &mut next_id, &mut rng);
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
        assert!(player.z > 180.0);

        // Continuous world: Eastbrook actors remain.
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("captain_alden")));
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("fen_crawler")));
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("warden_selene")));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::ZoneChanged { player: 1, zone_id } if zone_id == "eastfen"
        )));
    }

    #[test]
    fn eastfen_to_mirefen_preserves_player_progression() {
        let mut player = create_player(1, "Fenwalker", PlayerClass::Druid, 8.0, 304.0);
        player.zone_id = "eastfen".into();
        player.xp = 241;
        player.level = 5;
        player.talent_points = 3;
        player.talents.insert("natures_grace".into(), 2);
        assert!(grant_into(&mut player.inventory, "toad_bile", 3));

        let mut entities = vec![player];
        let mut next_id = 2;
        let mut rng = crate::rng::Rng::new(2);
        populate_all_overworld(&mut entities, &mut next_id, &mut rng);
        let mut events = Vec::new();

        assert!(enter_portal(&mut entities, 1, "mirefen", &mut events));

        let player = entities.iter().find(|entity| entity.id == 1).unwrap();
        assert_eq!(player.zone_id, "mirefen");
        assert_eq!(player.xp, 241);
        assert_eq!(count_item(&player.inventory, "toad_bile"), 3);
        assert_eq!(player.x, MIREFEN.player_spawn_x);
        assert_eq!(player.z, MIREFEN.player_spawn_z);
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("keeper_orla")));
        assert!(entities
            .iter()
            .any(|entity| entity.template_id.as_deref() == Some("mire_terror")));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::ZoneChanged { player: 1, zone_id } if zone_id == "mirefen"
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
