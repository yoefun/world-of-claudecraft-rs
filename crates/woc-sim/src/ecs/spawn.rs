//! World-native entity factories (columns only; no fat Entity).

use std::collections::HashMap;

use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Durable, Equipment, EquipmentWear, GatherNodeState,
    Health, Hearth, Home, Identity, InstanceAt, LootPile, LootTable, Motion, Owner, Progress,
    QuestLog, Reputation, Respawn, Riding, Skinnable, Spirit, Threat, Transform,
};
use crate::ecs::World;
use crate::inventory::grant_into;
use crate::types::{player_hp, BACKPACK_SLOTS, BANK_SLOTS};
use crate::world::{ground_height, WORLD_SEED};
use woc_content::{
    class_def, known_abilities_at_level, mob, mob_is_skinnable, npc, PetDef, PlayerClass,
    ResourceType, EASTBROOK,
};
use woc_protocol::{EntityId, EntityKind};

pub fn ground_at(x: f32, z: f32) -> f32 {
    ground_height(x, z, WORLD_SEED)
}

/// Adopt a factory's caller-supplied id, which must be fresh.
///
/// `World::next_id()` reserves nothing, so two reads with no adopt between them
/// return the same id and the second `adopt` reports `false`. The assert is
/// debug-only, but the `adopt` call itself must run in every build — writing
/// `debug_assert!(world.adopt(id))` would elide the adopt entirely under
/// `--release`, since `debug_assert!` does not evaluate its expression there.
pub(crate) fn adopt_fresh_id(world: &mut World, id: EntityId) {
    let adopted = world.adopt(id);
    debug_assert!(
        adopted,
        "factory id {id} is already live or zero; ids must be fresh"
    );
}

fn insert_identity(
    world: &mut World,
    id: EntityId,
    kind: EntityKind,
    name: &str,
    template_id: Option<&str>,
    zone_id: &str,
) {
    world.insert(
        id,
        Identity {
            kind,
            name: name.to_string(),
            template_id: template_id.map(|s| s.to_string()),
            zone_id: zone_id.to_string(),
        },
    );
}

fn insert_transform(world: &mut World, id: EntityId, x: f32, z: f32, yaw: f32) {
    let y = ground_at(x, z);
    world.insert(id, Transform { x, y, z, yaw });
}

fn insert_health(world: &mut World, id: EntityId, hp: f32, hp_max: f32, level: u32) {
    world.insert(
        id,
        Health {
            hp,
            hp_max,
            alive: true,
            level,
        },
    );
}

fn insert_combat_blank(world: &mut World, id: EntityId, attack_damage: f32) {
    world.insert(
        id,
        Combat {
            attack_damage,
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
    world.insert(id, Auras { auras: Vec::new() });
}

/// Refresh `known_abilities` on a player's ClassKit from class + level.
pub fn refresh_known_abilities(world: &mut World, player_id: EntityId) {
    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
    let Some(kit) = world.get_mut::<ClassKit>(player_id) else {
        return;
    };
    let Some(class) = class else {
        kit.known_abilities.clear();
        return;
    };
    kit.known_abilities = known_abilities_at_level(class, level)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
}

/// Adopt `id` and insert player columns (replaces fat `create_player`).
pub fn create_player(
    world: &mut World,
    id: EntityId,
    name: &str,
    class: PlayerClass,
    x: f32,
    z: f32,
) -> EntityId {
    let def = class_def(class);
    let hp = player_hp(def.base_hp, 1);
    adopt_fresh_id(world, id);
    insert_identity(
        world,
        id,
        EntityKind::Player,
        name,
        Some(def.id.as_str()),
        "eastbrook",
    );
    insert_transform(world, id, x, z, 0.0);
    insert_health(world, id, hp, hp, 1);
    insert_combat_blank(world, id, def.attack_power);
    let resource = match def.resource_type {
        ResourceType::Rage => 0.0,
        ResourceType::Mana | ResourceType::Energy => def.resource_max * 0.5,
    };
    world.insert(
        id,
        ClassKit {
            class_id: Some(class),
            resource,
            resource_max: def.resource_max,
            resource_type: Some(def.resource_type),
            primary_ability: Some(def.primary_ability.to_string()),
            known_abilities: Vec::new(),
            ability_cds: HashMap::new(),
            combo_points: 0,
            stealthed: false,
            stance_id: None,
        },
    );
    let mut inventory = vec![None; BACKPACK_SLOTS];
    for (item_id, count) in def.start_items {
        let _ = grant_into(&mut inventory, item_id, *count);
    }
    let equipment = Equipment {
        main_hand: Some(def.start_weapon.to_string()),
        chest: Some(def.start_chest.to_string()),
        head: Some("recruit_cap".into()),
        legs: Some("recruit_pants".into()),
        feet: Some("recruit_boots".into()),
        ..Default::default()
    };
    let equipment_wear = EquipmentWear::full_for_equipment(&equipment);
    world.insert(
        id,
        Bags {
            inventory,
            equipment,
            equipment_wear,
            equipment_enchants: Default::default(),
            equipment_qualities: Default::default(),
            open_vendor_npc: None,
            buyback: Vec::new(),
        },
    );
    world.insert(id, QuestLog::default());
    world.insert(id, Progress::default());
    world.insert(id, Reputation::default());
    world.insert(
        id,
        Bank {
            bank: vec![None; BANK_SLOTS],
            bank_copper: 0,
        },
    );
    let y = ground_at(x, z);
    world.insert(
        id,
        Motion {
            vx: 0.0,
            vz: 0.0,
            vy: 0.0,
            on_ground: true,
            jumping: false,
            fall_start_y: y,
            flying: false,
        },
    );
    world.insert(id, Spirit::default());
    world.insert(id, InstanceAt::default());
    world.insert(id, Durable::default());
    world.insert(
        id,
        Hearth {
            zone_id: "eastbrook".into(),
            x: EASTBROOK.player_spawn_x,
            z: EASTBROOK.player_spawn_z,
            ready_tick: 0,
        },
    );
    world.insert(id, Riding::default());
    refresh_known_abilities(world, id);
    crate::stats::recalc_player_stats(world, id);
    crate::combat::apply_spawn_identity(world, id);
    id
}

pub fn create_mob_from_template(
    world: &mut World,
    id: EntityId,
    template_id: &str,
    x: f32,
    z: f32,
) -> Option<EntityId> {
    let t = mob(template_id)?;
    adopt_fresh_id(world, id);
    insert_identity(world, id, EntityKind::Mob, t.name, Some(t.id), "eastbrook");
    insert_transform(world, id, x, z, 0.0);
    insert_health(world, id, t.hp, t.hp, t.level);
    insert_combat_blank(world, id, t.attack_damage);
    world.insert(
        id,
        Home {
            home_x: x,
            home_z: z,
        },
    );
    world.insert(id, Threat::default());
    world.insert(
        id,
        LootTable {
            loot_copper: 0,
            loot_item: None,
            xp_value: t.xp,
        },
    );
    world.insert(id, Respawn::default());
    world.insert(id, InstanceAt::default());
    Some(id)
}

pub fn create_npc_from_template(
    world: &mut World,
    id: EntityId,
    template_id: &str,
    x: f32,
    z: f32,
) -> Option<EntityId> {
    let t = npc(template_id)?;
    adopt_fresh_id(world, id);
    insert_identity(world, id, EntityKind::Npc, t.name, Some(t.id), "eastbrook");
    insert_transform(world, id, x, z, 0.0);
    insert_health(world, id, 1000.0, 1000.0, 1);
    Some(id)
}

pub fn create_loot(
    world: &mut World,
    id: EntityId,
    x: f32,
    z: f32,
    copper: u32,
    item: Option<String>,
) -> EntityId {
    adopt_fresh_id(world, id);
    insert_identity(world, id, EntityKind::Loot, "Loot", None, "eastbrook");
    insert_transform(world, id, x, z, 0.0);
    world.insert(
        id,
        LootPile {
            copper,
            item,
            quality: None,
        },
    );
    world.insert(id, InstanceAt::default());
    id
}

pub fn create_pet(
    world: &mut World,
    id: EntityId,
    def: &PetDef,
    owner_id: EntityId,
    x: f32,
    z: f32,
) -> EntityId {
    adopt_fresh_id(world, id);
    insert_identity(
        world,
        id,
        EntityKind::Pet,
        def.name,
        Some(def.id),
        "eastbrook",
    );
    insert_transform(world, id, x, z, 0.0);
    insert_health(world, id, def.hp, def.hp, def.level);
    world.insert(
        id,
        Combat {
            attack_damage: def.attack_damage,
            armor: 0.0,
            spell_power: 0.0,
            swing_timer: 0.0,
            ability_cd: 0.0,
            auto_attack: true,
            target: None,
            gcd: 0.0,
            cast: None,
            cast_lockout: 0.0,
        },
    );
    world.insert(id, Auras { auras: Vec::new() });
    world.insert(id, Owner { owner_id });
    world.insert(id, InstanceAt::default());
    if let Some(owner_inst) = world.get::<InstanceAt>(owner_id).cloned() {
        if let Some(slot) = world.get_mut::<InstanceAt>(id) {
            *slot = owner_inst;
        }
    }
    if let Some(zone) = world
        .get::<Identity>(owner_id)
        .map(|i| i.zone_id.clone())
    {
        if let Some(identity) = world.get_mut::<Identity>(id) {
            identity.zone_id = zone;
        }
    }
    id
}

/// Profession gather node (Loot kind + template; not auto-picked up).
pub fn create_gather_node(
    world: &mut World,
    id: EntityId,
    node: &woc_content::GatherNodeDef,
) -> EntityId {
    adopt_fresh_id(world, id);
    insert_identity(
        world,
        id,
        EntityKind::Loot,
        node.name,
        Some(node.id),
        node.zone_id,
    );
    insert_transform(world, id, node.x, node.z, 0.0);
    world.insert(
        id,
        LootPile {
            copper: 0,
            item: Some(node.item_id.to_string()),
            quality: None,
        },
    );
    world.insert(id, GatherNodeState { ready_tick: 0 });
    world.insert(id, InstanceAt::default());
    id
}

pub fn maybe_mark_skinnable(world: &mut World, loot_id: EntityId, template_id: &str) {
    if mob_is_skinnable(template_id) {
        world.insert(
            loot_id,
            Skinnable {
                tier: 1,
                skinned: false,
            },
        );
    }
}
