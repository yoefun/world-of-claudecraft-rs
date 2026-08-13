//! Dual-write: copy a fat `Entity` into sparse columns.

use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Durable, Health, Home, Identity, InstanceAt, LootPile,
    LootTable, Motion, Owner, Progress, QuestLog, Respawn, Spirit, Threat, Transform,
};
use crate::ecs::World;
use crate::entity::Entity;
use woc_protocol::EntityKind;

/// Insert columns for `entity` (adopting its id). Kind decides which columns exist.
pub fn sync_entity_to_world(world: &mut World, entity: &Entity) {
    world.adopt(entity.id);
    world.insert(
        entity.id,
        Identity {
            kind: entity.kind,
            name: entity.name.clone(),
            template_id: entity.template_id.clone(),
            zone_id: entity.zone_id.clone(),
        },
    );
    world.insert(
        entity.id,
        Transform {
            x: entity.x,
            y: entity.y,
            z: entity.z,
            yaw: entity.yaw,
        },
    );

    match entity.kind {
        EntityKind::Player => sync_player(world, entity),
        EntityKind::Mob => sync_mob(world, entity),
        EntityKind::Npc => sync_npc(world, entity),
        EntityKind::Loot => sync_loot(world, entity),
        EntityKind::Pet => sync_pet(world, entity),
    }
}

fn sync_health(world: &mut World, entity: &Entity) {
    world.insert(
        entity.id,
        Health {
            hp: entity.hp,
            hp_max: entity.hp_max,
            alive: entity.alive,
            level: entity.level,
        },
    );
}

fn sync_combat(world: &mut World, entity: &Entity) {
    world.insert(
        entity.id,
        Combat {
            attack_damage: entity.attack_damage,
            armor: entity.armor,
            swing_timer: entity.swing_timer,
            ability_cd: entity.ability_cd,
            auto_attack: entity.auto_attack,
            target: entity.target,
            gcd: entity.gcd,
            cast: entity.cast.clone(),
        },
    );
    world.insert(
        entity.id,
        Auras {
            auras: entity.auras.clone(),
        },
    );
}

fn sync_player(world: &mut World, entity: &Entity) {
    sync_health(world, entity);
    sync_combat(world, entity);
    world.insert(
        entity.id,
        ClassKit {
            class_id: entity.class_id,
            resource: entity.resource,
            resource_max: entity.resource_max,
            resource_type: entity.resource_type,
            primary_ability: entity.primary_ability.clone(),
            known_abilities: entity.known_abilities.clone(),
            ability_cds: entity.ability_cds.clone(),
        },
    );
    world.insert(
        entity.id,
        Bags {
            inventory: entity.inventory.clone(),
            equipment: entity.equipment.clone(),
            open_vendor_npc: entity.open_vendor_npc,
        },
    );
    world.insert(
        entity.id,
        QuestLog {
            quest_log: entity.quest_log.clone(),
        },
    );
    world.insert(
        entity.id,
        Progress {
            xp: entity.xp,
            copper: entity.copper,
            talent_points: entity.talent_points,
            talents: entity.talents.clone(),
            honor: entity.honor,
            pvp_flagged: entity.pvp_flagged,
            professions: entity.professions.clone(),
            completed_deeds: entity.completed_deeds.clone(),
        },
    );
    world.insert(
        entity.id,
        Bank {
            bank: entity.bank.clone(),
        },
    );
    world.insert(
        entity.id,
        Motion {
            vx: entity.vx,
            vz: entity.vz,
            vy: entity.vy,
            on_ground: entity.on_ground,
            jumping: entity.jumping,
            fall_start_y: entity.fall_start_y,
            flying: entity.flying,
        },
    );
    world.insert(
        entity.id,
        Spirit {
            corpse_x: entity.corpse_x,
            corpse_z: entity.corpse_z,
        },
    );
    world.insert(
        entity.id,
        InstanceAt {
            instance_id: entity.instance_id.clone(),
            delve_room: entity.delve_room,
        },
    );
    world.insert(
        entity.id,
        Durable {
            durable_id: entity.durable_id.clone(),
        },
    );
}

fn sync_mob(world: &mut World, entity: &Entity) {
    sync_health(world, entity);
    sync_combat(world, entity);
    world.insert(
        entity.id,
        Home {
            home_x: entity.home_x,
            home_z: entity.home_z,
        },
    );
    world.insert(
        entity.id,
        Threat {
            threat: entity.threat.clone(),
        },
    );
    world.insert(
        entity.id,
        LootTable {
            loot_copper: entity.loot_copper,
            loot_item: entity.loot_item.clone(),
            xp_value: entity.xp_value,
        },
    );
    world.insert(
        entity.id,
        Respawn {
            respawn_timer: entity.respawn_timer,
        },
    );
}

fn sync_npc(world: &mut World, entity: &Entity) {
    sync_health(world, entity);
}

fn sync_loot(world: &mut World, entity: &Entity) {
    world.insert(
        entity.id,
        LootPile {
            copper: entity.loot_copper,
            item: entity.loot_item.clone(),
        },
    );
}

fn sync_pet(world: &mut World, entity: &Entity) {
    sync_health(world, entity);
    sync_combat(world, entity);
    if let Some(owner_id) = entity.owner_id {
        world.insert(entity.id, Owner { owner_id });
    }
}
