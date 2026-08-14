//! Overworld zone transitions on the continuous strip.

use crate::ecs::components::{Bags, Combat, Hearth, Identity, InstanceAt, Threat, Transform};
use crate::ecs::World;
use crate::types::HEARTH_COOLDOWN_TICKS;
use crate::world::WORLD_SEED;
use woc_content::{ZoneLayout, EASTBROOK, EASTFEN, GATHER_NODES, MIREFEN, THORNPEAK};
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

fn zone_population_seed(tag: &str) -> u32 {
    let mut h = WORLD_SEED;
    for b in tag.as_bytes() {
        h = h.wrapping_mul(16777619) ^ u32::from(*b);
    }
    if h == 0 {
        0x9e3779b9
    } else {
        h
    }
}

/// Teleport through a portal without wiping other-zone actors.
pub fn enter_portal(
    world: &mut World,
    player_id: EntityId,
    zone_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    if !load_overworld_zone(world, player_id, zone_id) {
        return false;
    }

    events.push(SimEvent::ZoneChanged {
        player: player_id,
        zone_id: layout_zone_tag(zone_id).to_string(),
    });
    true
}

/// Ensure the destination zone population exists, then teleport the player.
pub(crate) fn load_overworld_zone(world: &mut World, player_id: EntityId, zone_id: &str) -> bool {
    let Some(layout) = zone_layout(zone_id) else {
        return false;
    };
    load_overworld_zone_at(
        world,
        player_id,
        zone_id,
        layout.player_spawn_x,
        layout.player_spawn_z,
    )
}

/// Ensure the destination zone population exists, then teleport to explicit coordinates.
pub(crate) fn load_overworld_zone_at(
    world: &mut World,
    player_id: EntityId,
    zone_id: &str,
    x: f32,
    z: f32,
) -> bool {
    let Some(layout) = zone_layout(zone_id) else {
        return false;
    };
    if world.get::<Identity>(player_id).map(|i| i.kind) != Some(EntityKind::Player) {
        return false;
    }
    let old_instance_id = world
        .get::<InstanceAt>(player_id)
        .and_then(|instance| instance.instance_id.clone());

    let tag = layout_zone_tag(zone_id);
    let mut rng = crate::rng::Rng::new(zone_population_seed(tag));
    ensure_zone_population(world, layout, tag, &mut rng);

    let y = crate::ecs::spawn::ground_at(x, z);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.x = x;
        t.z = z;
        t.y = y;
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
        bags.buyback.clear();
    }
    if let Some(threat) = world.get_mut::<Threat>(player_id) {
        threat.threat.clear();
    }
    crate::instances::follow_owner_into_instance(world, player_id);
    if let Some(instance_id) = old_instance_id {
        crate::instances::despawn_instance_if_empty(world, &instance_id);
    }
    true
}

pub(crate) fn use_hearthstone(
    world: &mut World,
    player_id: EntityId,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(hearth) = world.get::<Hearth>(player_id).cloned() else {
        return false;
    };
    if tick < hearth.ready_tick {
        events.push(SimEvent::Toast {
            message: "Hearthstone is not ready.".into(),
        });
        return false;
    }
    if !load_overworld_zone_at(world, player_id, &hearth.zone_id, hearth.x, hearth.z) {
        return false;
    }
    if let Some(hearth) = world.get_mut::<Hearth>(player_id) {
        hearth.ready_tick = tick + HEARTH_COOLDOWN_TICKS;
    }
    true
}

fn ensure_zone_population(
    world: &mut World,
    layout: &ZoneLayout,
    tag: &str,
    rng: &mut crate::rng::Rng,
) {
    let has_zone_npc = world.ids::<Identity>().into_iter().any(|id| {
        world.get::<Identity>(id).is_some_and(|identity| {
            identity.kind == EntityKind::Npc
                && identity.zone_id == tag
                && identity.template_id.is_some()
        })
    });
    if has_zone_npc {
        return;
    }
    for spot in layout.npcs {
        let id = world.next_id();
        if let Some(nid) =
            crate::ecs::spawn::create_npc_from_template(world, id, spot.npc_id, spot.x, spot.z)
        {
            if let Some(identity) = world.get_mut::<Identity>(nid) {
                identity.zone_id = tag.to_string();
            }
        }
    }
    for spot in layout.mobs {
        for i in 0..spot.count {
            let id = world.next_id();
            let (x, z) = if spot.radius > 0.0 {
                let ox = (rng.next_f32() - 0.5) * 2.0 * spot.radius;
                let oz = (rng.next_f32() - 0.5) * 2.0 * spot.radius;
                (spot.x + ox, spot.z + oz)
            } else {
                (spot.x + i as f32 * 1.2, spot.z)
            };
            if let Some(mid) =
                crate::ecs::spawn::create_mob_from_template(world, id, spot.mob_id, x, z)
            {
                if let Some(identity) = world.get_mut::<Identity>(mid) {
                    identity.zone_id = tag.to_string();
                }
            }
        }
    }
}

/// Populate all overworld layouts into one continuous realm.
pub fn populate_all_overworld(world: &mut World, rng: &mut crate::rng::Rng) {
    for (layout, tag) in [
        (&EASTBROOK, "eastbrook"),
        (&EASTFEN, "eastfen"),
        (&MIREFEN, "mirefen"),
        (&THORNPEAK, "thornpeak"),
    ] {
        for spot in layout.npcs {
            let id = world.next_id();
            if let Some(nid) =
                crate::ecs::spawn::create_npc_from_template(world, id, spot.npc_id, spot.x, spot.z)
            {
                if let Some(identity) = world.get_mut::<Identity>(nid) {
                    identity.zone_id = tag.to_string();
                }
            }
        }
        for spot in layout.mobs {
            for i in 0..spot.count {
                let id = world.next_id();
                let (x, z) = if spot.radius > 0.0 {
                    let ox = (rng.next_f32() - 0.5) * 2.0 * spot.radius;
                    let oz = (rng.next_f32() - 0.5) * 2.0 * spot.radius;
                    (spot.x + ox, spot.z + oz)
                } else {
                    (spot.x + i as f32 * 1.2, spot.z)
                };
                if let Some(mid) =
                    crate::ecs::spawn::create_mob_from_template(world, id, spot.mob_id, x, z)
                {
                    if let Some(identity) = world.get_mut::<Identity>(mid) {
                        identity.zone_id = tag.to_string();
                    }
                }
            }
        }
    }
    spawn_gather_nodes(world);
}

/// Place profession gather nodes as world entities (loot-kind + gather template).
pub fn spawn_gather_nodes(world: &mut World) {
    for node in GATHER_NODES {
        let exists = world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some(node.id)
        });
        if exists {
            continue;
        }
        let id = world.next_id();
        crate::ecs::spawn::create_gather_node(world, id, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Health, Progress};
    use crate::inventory::{count_item, grant_into};
    use woc_content::PlayerClass;

    fn player_world(name: &str, class: PlayerClass, x: f32, z: f32) -> (World, EntityId) {
        let mut world = World::new();
        let id = crate::ecs::spawn::create_player(&mut world, 1, name, class, x, z);
        (world, id)
    }

    #[test]
    fn eastbrook_to_eastfen_preserves_player_progression() {
        let (mut world, pid) = player_world("Traveler", PlayerClass::Mage, 30.0, 30.0);
        if let Some(p) = world.get_mut::<Progress>(pid) {
            p.xp = 73;
            p.talent_points = 2;
            p.talents.insert("arcane_focus".into(), 1);
        }
        if let Some(h) = world.get_mut::<Health>(pid) {
            h.level = 2;
        }
        if let Some(bags) = world.get_mut::<Bags>(pid) {
            assert!(grant_into(&mut bags.inventory, "wolf_fang", 2));
        }

        let mut rng = crate::rng::Rng::new(1);
        populate_all_overworld(&mut world, &mut rng);
        let mut events = Vec::new();

        assert!(enter_portal(&mut world, pid, "eastfen", &mut events));

        assert_eq!(world.get::<Identity>(pid).unwrap().zone_id, "eastfen");
        let progress = world.get::<Progress>(pid).unwrap();
        assert_eq!(progress.xp, 73);
        assert_eq!(world.get::<Health>(pid).unwrap().level, 2);
        assert_eq!(progress.talent_points, 2);
        assert_eq!(progress.talents.get("arcane_focus"), Some(&1));
        assert_eq!(
            count_item(&world.get::<Bags>(pid).unwrap().inventory, "wolf_fang"),
            2
        );
        let t = world.get::<Transform>(pid).unwrap();
        assert_eq!(t.x, EASTFEN.player_spawn_x);
        assert_eq!(t.z, EASTFEN.player_spawn_z);
        assert!(t.z > 180.0);

        assert!(world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some("captain_alden")
        }));
        assert!(world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some("fen_crawler")
        }));
    }

    #[test]
    fn mirefen_portal_preserves_progress_and_keeps_eastbrook() {
        let (mut world, pid) = player_world("Fenwalker", PlayerClass::Druid, 8.0, 304.0);
        if let Some(p) = world.get_mut::<Progress>(pid) {
            p.xp = 40;
            p.copper = 12;
        }
        if let Some(h) = world.get_mut::<Health>(pid) {
            h.level = 3;
        }
        if let Some(bags) = world.get_mut::<Bags>(pid) {
            assert!(grant_into(&mut bags.inventory, "toad_bile", 3));
        }
        let mut rng = crate::rng::Rng::new(2);
        populate_all_overworld(&mut world, &mut rng);
        let mut events = Vec::new();
        assert!(enter_portal(&mut world, pid, "mirefen", &mut events));
        assert_eq!(world.get::<Identity>(pid).unwrap().zone_id, "mirefen");
        assert_eq!(world.get::<Progress>(pid).unwrap().xp, 40);
        assert_eq!(
            count_item(&world.get::<Bags>(pid).unwrap().inventory, "toad_bile"),
            3
        );
        assert!(world.ids::<Identity>().into_iter().any(|id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some("captain_alden")
        }));
    }

    #[test]
    fn unsupported_zone_does_not_mutate_player() {
        let (mut world, pid) = player_world("Traveler", PlayerClass::Warrior, 9.0, 11.0);
        let mut events = Vec::new();
        assert!(!enter_portal(&mut world, pid, "nope", &mut events));
        let t = world.get::<Transform>(pid).unwrap();
        assert_eq!((t.x, t.z), (9.0, 11.0));
    }

    #[test]
    fn populate_spawns_wolf_run_pack() {
        let mut world = World::new();
        let mut rng = crate::rng::Rng::new(1);
        populate_all_overworld(&mut world, &mut rng);
        let n = world
            .ids::<Identity>()
            .into_iter()
            .filter(|&id| {
                world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some("young_wolf")
                    && world.get::<Identity>(id).map(|i| i.zone_id.as_str()) == Some("eastbrook")
                    && world.get::<Health>(id).is_some_and(|h| h.alive)
            })
            .count();
        assert!(n >= 5, "eastbrook young_wolf count={n}");
    }

    #[test]
    fn gather_nodes_spawn_once() {
        let mut world = World::new();
        let mut rng = crate::rng::Rng::new(1);
        populate_all_overworld(&mut world, &mut rng);
        let before = world
            .ids::<Identity>()
            .into_iter()
            .filter(|&id| {
                world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_ref())
                    .and_then(|tid| woc_content::gather_node(tid))
                    .is_some()
            })
            .count();
        assert!(before >= GATHER_NODES.len());
        spawn_gather_nodes(&mut world);
        let after = world
            .ids::<Identity>()
            .into_iter()
            .filter(|&id| {
                world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_ref())
                    .and_then(|tid| woc_content::gather_node(tid))
                    .is_some()
            })
            .count();
        assert_eq!(before, after);
    }

    #[test]
    fn zone_population_seed_differs_for_equal_length_tags() {
        let a = zone_population_seed("eastfen");
        let b = zone_population_seed("mirefen");
        assert_ne!(
            a, b,
            "equal-length zone tags must not share a portal population seed"
        );
        assert_ne!(zone_population_seed("eastbrook"), a);
    }
}
