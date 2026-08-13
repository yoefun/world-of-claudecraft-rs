//! Overworld zone transitions on the continuous strip.

use crate::ecs::components::{Bags, Combat, Identity, InstanceAt, Threat, Transform};
use crate::ecs::World;
use crate::entity::{create_mob_from_template, create_npc_from_template, Entity};
use woc_content::{
    GatherNodeDef, ZoneLayout, EASTBROOK, EASTFEN, GATHER_NODES, MIREFEN, THORNPEAK,
};
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
    world: &mut World,
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    zone_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    if !load_overworld_zone(world, entities, player_id, zone_id) {
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
    world: &mut World,
    entities: &mut Vec<Entity>,
    player_id: EntityId,
    zone_id: &str,
) -> bool {
    let Some(layout) = zone_layout(zone_id) else {
        return false;
    };
    if world
        .get::<Identity>(player_id)
        .map(|i| i.kind)
        != Some(EntityKind::Player)
    {
        return false;
    }

    let tag = layout_zone_tag(zone_id);
    ensure_zone_population(world, entities, layout, tag);

    let spawn_y = Entity::ground_at(layout.player_spawn_x, layout.player_spawn_z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = layout.player_spawn_x;
        t.z = layout.player_spawn_z;
        t.y = spawn_y;
    }
    if let Some(identity) = world.get_mut::<Identity>(player_id) {
        identity.zone_id = tag.to_string();
    }
    if let Some(inst) = world.get_mut::<InstanceAt>(player_id) {
        inst.instance_id = None;
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

    if let Some(entity) = entities.iter_mut().find(|e| e.id == player_id) {
        crate::ecs::spawn::apply_world_to_entity(world, entity);
    }
    true
}

fn ensure_zone_population(
    world: &mut World,
    entities: &mut Vec<Entity>,
    layout: &ZoneLayout,
    tag: &str,
) {
    let has_zone_npc = world.ids::<Identity>().into_iter().any(|id| {
        world
            .get::<Identity>(id)
            .is_some_and(|identity| {
                identity.kind == EntityKind::Npc
                    && identity.zone_id == tag
                    && identity.template_id.is_some()
            })
    });
    if has_zone_npc {
        return;
    }
    let mut next_id = next_entity_id(entities, world);
    for spot in layout.npcs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut npc) = create_npc_from_template(id, spot.npc_id, spot.x, spot.z) {
            npc.zone_id = tag.to_string();
            crate::ecs::spawn::sync_entity_to_world(world, &npc);
            entities.push(npc);
        }
    }
    for spot in layout.mobs {
        let id = next_id;
        next_id = next_id.saturating_add(1);
        if let Some(mut mob) = create_mob_from_template(id, spot.mob_id, spot.x, spot.z) {
            mob.zone_id = tag.to_string();
            crate::ecs::spawn::sync_entity_to_world(world, &mob);
            entities.push(mob);
        }
    }
    world.set_next_id(next_id);
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
    spawn_gather_nodes(entities, next_id);
}

/// Place profession gather nodes as world entities (loot-kind + gather template).
pub fn spawn_gather_nodes(entities: &mut Vec<Entity>, next_id: &mut EntityId) {
    for node in GATHER_NODES {
        if entities
            .iter()
            .any(|e| e.template_id.as_deref() == Some(node.id))
        {
            continue;
        }
        let id = *next_id;
        *next_id = next_id.saturating_add(1);
        entities.push(gather_entity(id, node));
    }
}

fn gather_entity(id: EntityId, node: &GatherNodeDef) -> Entity {
    let mut e = Entity::blank(
        id,
        EntityKind::Loot,
        node.name,
        Some(node.id),
        node.x,
        node.z,
    );
    e.zone_id = node.zone_id.to_string();
    e.loot_item = Some(node.item_id.to_string());
    e.loot_copper = 0;
    e
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
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        let mut events = Vec::new();

        assert!(enter_portal(
            &mut world,
            &mut entities,
            1,
            "eastfen",
            &mut events
        ));

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
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        let mut events = Vec::new();

        assert!(enter_portal(
            &mut world,
            &mut entities,
            1,
            "mirefen",
            &mut events
        ));

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
    fn populate_spawns_gather_nodes() {
        let mut entities = Vec::new();
        let mut next_id = 1;
        let mut rng = crate::rng::Rng::new(1);
        populate_all_overworld(&mut entities, &mut next_id, &mut rng);
        assert!(entities
            .iter()
            .any(|e| e.template_id.as_deref() == Some("eastbrook_meadow_silverleaf")));
        assert!(entities
            .iter()
            .any(|e| e.template_id.as_deref() == Some("eastbrook_brook_peacebloom")));
        let herbs = entities
            .iter()
            .filter(|e| {
                e.kind == EntityKind::Loot
                    && e.template_id
                        .as_deref()
                        .is_some_and(|t| t.contains("eastbrook_"))
            })
            .count();
        assert!(herbs >= 3, "expected gather herbs, got {herbs}");
    }

    #[test]
    fn unsupported_zone_does_not_mutate_player() {
        let player = create_player(1, "Traveler", PlayerClass::Warrior, 9.0, 11.0);
        let mut entities = vec![player];
        let mut world = crate::ecs::spawn::world_from_entities(&entities);
        let mut events = Vec::new();

        assert!(!enter_portal(
            &mut world,
            &mut entities,
            1,
            "missing_zone",
            &mut events
        ));
        assert_eq!((entities[0].x, entities[0].z), (9.0, 11.0));
        assert!(events.is_empty());
    }
}
