//! Combat: auto-attack, primary ability, damage, death, XP, loot.

use crate::entity::{create_loot, Entity};
use crate::rng::Rng;
use crate::types::{
    player_hp, xp_to_next, MELEE_RANGE, MOB_SWING_SEC, PLAYER_SWING_SEC, RANGED_FALLBACK,
};
use woc_content::{ability, class_def, mob, ResourceType};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, SimEvent, DT};

pub fn dist2d(a: &Entity, b: &Entity) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

pub fn face_toward(from: &Entity, to: &Entity) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

fn gain_resource(player: &mut Entity, amount: f32) {
    player.resource = (player.resource + amount).min(player.resource_max);
}

fn spend_resource(player: &mut Entity, amount: f32) -> bool {
    if player.resource + 1e-3 < amount {
        return false;
    }
    player.resource -= amount;
    true
}

pub fn deal_damage(
    entities: &mut [Entity],
    source: EntityId,
    target: EntityId,
    amount: f32,
    ability_name: Option<&str>,
    events: &mut Vec<SimEvent>,
) {
    let Some(ti) = entities.iter().position(|e| e.id == target) else {
        return;
    };
    if !entities[ti].alive || entities[ti].kind == EntityKind::Npc {
        return;
    }
    let mitigated = (amount - entities[ti].armor * 0.05).max(1.0);
    entities[ti].hp = (entities[ti].hp - mitigated).max(0.0);
    events.push(SimEvent::Damage {
        source,
        target,
        amount: mitigated,
        ability: ability_name.map(|s| s.to_string()),
    });
    if entities[ti].hp <= 0.0 {
        entities[ti].alive = false;
        let victim_name = entities[ti].name.clone();
        events.push(SimEvent::Kill {
            killer: source,
            victim: target,
            victim_name,
        });
    }
}

pub struct KillReward {
    pub victim: EntityId,
    pub template_id: Option<String>,
    pub x: f32,
    pub z: f32,
    pub xp: u32,
}

pub fn collect_pending_mob_kills(events: &[SimEvent], entities: &[Entity]) -> Vec<KillReward> {
    let mut out = Vec::new();
    for ev in events {
        if let SimEvent::Kill { victim, .. } = ev {
            if let Some(e) = entities.iter().find(|e| e.id == *victim) {
                if e.kind == EntityKind::Mob {
                    out.push(KillReward {
                        victim: *victim,
                        template_id: e.template_id.clone(),
                        x: e.x,
                        z: e.z,
                        xp: e.xp_value,
                    });
                }
            }
        }
    }
    out
}

pub fn grant_xp(player: &mut Entity, xp: &mut u32, amount: u32, events: &mut Vec<SimEvent>) {
    *xp = xp.saturating_add(amount);
    loop {
        let need = xp_to_next(player.level);
        if *xp < need {
            break;
        }
        *xp -= need;
        player.level += 1;
        if let Some(class) = player.class_id {
            let def = class_def(class);
            player.hp_max = player_hp(def.base_hp, player.level) + player.armor * 0.5;
        }
        player.hp = player.hp_max;
        events.push(SimEvent::LevelUp {
            player: player.id,
            level: player.level,
        });
        events.push(SimEvent::Toast {
            message: format!("You reached level {}!", player.level),
        });
    }
}

pub fn spawn_mob_loot(
    next_id: &mut EntityId,
    entities: &mut Vec<Entity>,
    rng: &mut Rng,
    template_id: Option<&str>,
    x: f32,
    z: f32,
) -> EntityId {
    let (copper, item) = if let Some(tid) = template_id.and_then(mob) {
        let copper = rng.gen_range_u32(tid.copper_min, tid.copper_max);
        let mut dropped = None;
        for entry in tid.loot {
            if rng.next_f32() < entry.chance {
                dropped = Some(entry.item_id.to_string());
                break;
            }
        }
        (copper, dropped)
    } else {
        (rng.gen_range_u32(3, 8), None)
    };
    let id = *next_id;
    *next_id += 1;
    entities.push(create_loot(id, x, z, copper, item));
    id
}

pub fn try_pickup_loot(
    player_id: EntityId,
    entities: &mut [Entity],
    copper: &mut u32,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    let loot_ids: Vec<EntityId> = entities
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            *i != pi
                && e.kind == EntityKind::Loot
                && e.alive
                && dist2d(&entities[pi], e) < crate::types::LOOT_RANGE
        })
        .map(|(_, e)| e.id)
        .collect();
    for lid in loot_ids {
        let Some(li) = entities.iter().position(|e| e.id == lid) else {
            continue;
        };
        let c = entities[li].loot_copper;
        let item = entities[li].loot_item.clone();
        if let Some(ref it) = item {
            if crate::inventory::grant_item(&mut entities[pi], it, 1, events).is_err() {
                events.push(SimEvent::Toast {
                    message: "Inventory full.".into(),
                });
                continue;
            }
            crate::quests::on_inventory_changed(&mut entities[pi], events);
        }
        entities[li].alive = false;
        *copper = copper.saturating_add(c);
        events.push(SimEvent::Loot {
            player: player_id,
            copper: c,
            item,
        });
    }
}

fn ability_range(player: &Entity) -> f32 {
    player
        .primary_ability
        .as_deref()
        .and_then(ability)
        .map(|a| a.range)
        .unwrap_or(MELEE_RANGE)
}

pub fn update_player_combat(
    player_id: EntityId,
    entities: &mut [Entity],
    ability_slot: Option<AbilitySlot>,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    if !entities[pi].alive {
        return;
    }

    // Soft regen for mana/energy out of swings.
    if let Some(ResourceType::Mana | ResourceType::Energy) = entities[pi].resource_type {
        gain_resource(&mut entities[pi], 1.5 * DT);
    }

    if entities[pi].ability_cd > 0.0 {
        entities[pi].ability_cd = (entities[pi].ability_cd - DT).max(0.0);
    }

    let target_id = entities[pi].target;
    let Some(tid) = target_id else {
        entities[pi].swing_timer = 0.0;
        return;
    };
    let Some(ti) = entities.iter().position(|e| e.id == tid) else {
        entities[pi].target = None;
        return;
    };
    if !entities[ti].alive || entities[ti].kind != EntityKind::Mob {
        entities[pi].target = None;
        entities[pi].auto_attack = false;
        return;
    }

    let range = ability_range(&entities[pi]).max(MELEE_RANGE);
    let d = dist2d(&entities[pi], &entities[ti]);
    let in_melee = d <= MELEE_RANGE;
    let in_ability = d <= range.max(RANGED_FALLBACK.min(range));

    entities[pi].yaw = face_toward(&entities[pi], &entities[ti]);

    // Primary ability (instant).
    if matches!(ability_slot, Some(AbilitySlot::Primary)) && entities[pi].ability_cd <= 0.0 {
        if let Some(abil_id) = entities[pi].primary_ability.clone() {
            if let Some(def) = ability(&abil_id) {
                if in_ability && spend_resource(&mut entities[pi], def.cost) {
                    entities[pi].ability_cd = def.cooldown;
                    entities[pi].auto_attack = true;
                    let dmg = def.damage + entities[pi].attack_damage * 0.35;
                    let src = entities[pi].id;
                    deal_damage(entities, src, tid, dmg, Some(def.name), events);
                    if matches!(entities[pi].resource_type, Some(ResourceType::Rage)) {
                        gain_resource(&mut entities[pi], 5.0);
                    }
                    return;
                }
            }
        }
    }

    if !entities[pi].auto_attack || !in_melee {
        return;
    }

    entities[pi].swing_timer -= DT;
    if entities[pi].swing_timer > 0.0 {
        return;
    }
    entities[pi].swing_timer = PLAYER_SWING_SEC;
    let dmg = entities[pi].attack_damage.max(4.0);
    if matches!(entities[pi].resource_type, Some(ResourceType::Rage)) {
        gain_resource(&mut entities[pi], 5.0);
    }
    let src = entities[pi].id;
    deal_damage(entities, src, tid, dmg, None, events);
}

pub fn update_mob_combat(
    mob_id: EntityId,
    player_id: EntityId,
    entities: &mut [Entity],
    events: &mut Vec<SimEvent>,
) {
    let Some(mi) = entities.iter().position(|e| e.id == mob_id) else {
        return;
    };
    if !entities[mi].alive || entities[mi].kind != EntityKind::Mob {
        return;
    }
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    if !entities[pi].alive {
        entities[mi].target = None;
        return;
    }

    let d = dist2d(&entities[mi], &entities[pi]);
    if d > MELEE_RANGE {
        return;
    }
    entities[mi].yaw = face_toward(&entities[mi], &entities[pi]);
    entities[mi].swing_timer -= DT;
    if entities[mi].swing_timer > 0.0 {
        return;
    }
    entities[mi].swing_timer = MOB_SWING_SEC;
    let dmg = entities[mi].attack_damage.max(3.0);
    let src = entities[mi].id;
    deal_damage(entities, src, player_id, dmg, None, events);
}
