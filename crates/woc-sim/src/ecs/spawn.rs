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

/// Copy columns back onto a fat `Entity` (combat writes World first).
pub fn apply_world_to_entity(world: &World, entity: &mut Entity) {
    let id = entity.id;
    if let Some(identity) = world.get::<Identity>(id) {
        entity.kind = identity.kind;
        entity.name.clone_from(&identity.name);
        entity.template_id.clone_from(&identity.template_id);
        entity.zone_id.clone_from(&identity.zone_id);
    }
    if let Some(t) = world.get::<Transform>(id) {
        entity.x = t.x;
        entity.y = t.y;
        entity.z = t.z;
        entity.yaw = t.yaw;
    }
    if let Some(h) = world.get::<Health>(id) {
        entity.hp = h.hp;
        entity.hp_max = h.hp_max;
        entity.alive = h.alive;
        entity.level = h.level;
    }
    if let Some(c) = world.get::<Combat>(id) {
        entity.attack_damage = c.attack_damage;
        entity.armor = c.armor;
        entity.swing_timer = c.swing_timer;
        entity.ability_cd = c.ability_cd;
        entity.auto_attack = c.auto_attack;
        entity.target = c.target;
        entity.gcd = c.gcd;
        entity.cast = c.cast.clone();
    }
    if let Some(a) = world.get::<Auras>(id) {
        entity.auras.clone_from(&a.auras);
    }
    if let Some(h) = world.get::<Home>(id) {
        entity.home_x = h.home_x;
        entity.home_z = h.home_z;
    }
    if let Some(t) = world.get::<Threat>(id) {
        entity.threat.clone_from(&t.threat);
    }
    if let Some(l) = world.get::<LootTable>(id) {
        entity.loot_copper = l.loot_copper;
        entity.loot_item.clone_from(&l.loot_item);
        entity.xp_value = l.xp_value;
    }
    if let Some(r) = world.get::<Respawn>(id) {
        entity.respawn_timer = r.respawn_timer;
    }
    if let Some(l) = world.get::<LootPile>(id) {
        entity.loot_copper = l.copper;
        entity.loot_item.clone_from(&l.item);
    }
    if let Some(o) = world.get::<Owner>(id) {
        entity.owner_id = Some(o.owner_id);
    }
    if let Some(k) = world.get::<ClassKit>(id) {
        entity.class_id = k.class_id;
        entity.resource = k.resource;
        entity.resource_max = k.resource_max;
        entity.resource_type = k.resource_type;
        entity.primary_ability.clone_from(&k.primary_ability);
        entity.known_abilities.clone_from(&k.known_abilities);
        entity.ability_cds.clone_from(&k.ability_cds);
    }
    if let Some(b) = world.get::<Bags>(id) {
        entity.inventory.clone_from(&b.inventory);
        entity.equipment.clone_from(&b.equipment);
        entity.open_vendor_npc = b.open_vendor_npc;
    }
    if let Some(q) = world.get::<QuestLog>(id) {
        entity.quest_log.clone_from(&q.quest_log);
    }
    if let Some(p) = world.get::<Progress>(id) {
        entity.xp = p.xp;
        entity.copper = p.copper;
        entity.talent_points = p.talent_points;
        entity.talents.clone_from(&p.talents);
        entity.honor = p.honor;
        entity.pvp_flagged = p.pvp_flagged;
        entity.professions.clone_from(&p.professions);
        entity.completed_deeds.clone_from(&p.completed_deeds);
    }
    if let Some(b) = world.get::<Bank>(id) {
        entity.bank.clone_from(&b.bank);
    }
    if let Some(m) = world.get::<Motion>(id) {
        entity.vx = m.vx;
        entity.vz = m.vz;
        entity.vy = m.vy;
        entity.on_ground = m.on_ground;
        entity.jumping = m.jumping;
        entity.fall_start_y = m.fall_start_y;
        entity.flying = m.flying;
    }
    if let Some(s) = world.get::<Spirit>(id) {
        entity.corpse_x = s.corpse_x;
        entity.corpse_z = s.corpse_z;
    }
    if let Some(i) = world.get::<InstanceAt>(id) {
        entity.instance_id.clone_from(&i.instance_id);
        entity.delve_room = i.delve_room;
    }
    if let Some(d) = world.get::<Durable>(id) {
        entity.durable_id.clone_from(&d.durable_id);
    }
}

pub fn apply_world_to_entities(world: &World, entities: &mut [Entity]) {
    for entity in entities {
        apply_world_to_entity(world, entity);
    }
}

/// Dual-write helper: adopt every fat entity into a fresh `World`.
pub fn world_from_entities(entities: &[Entity]) -> World {
    let mut world = World::new();
    for entity in entities {
        sync_entity_to_world(&mut world, entity);
    }
    world
}
