//! Combat: auto-attack, Heroic Strike, damage, death, XP, loot.

use crate::entity::{create_loot, Entity};
use crate::rng::Rng;
use crate::types::{
    warrior_hp, xp_to_next, HEROIC_STRIKE_BONUS, HEROIC_STRIKE_CD, HEROIC_STRIKE_COST, MELEE_RANGE,
    WARRIOR_RAGE_MAX, WARRIOR_SWING_SEC, WARRIOR_WEAPON_DAMAGE, WOLF_COPPER_MAX, WOLF_COPPER_MIN,
    WOLF_DAMAGE, WOLF_SWING_SEC, WOLF_XP,
};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, SimEvent, DT};

pub fn dist2d(a: &Entity, b: &Entity) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

pub fn face_toward(from: &Entity, to: &Entity) -> f32 {
    (to.x - from.x).atan2(to.z - from.z)
}

fn gain_rage(player: &mut Entity, amount: f32) {
    player.resource = (player.resource + amount).min(WARRIOR_RAGE_MAX);
}

fn spend_rage(player: &mut Entity, amount: f32) -> bool {
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
    ability: Option<&str>,
    events: &mut Vec<SimEvent>,
) {
    let Some(ti) = entities.iter().position(|e| e.id == target) else {
        return;
    };
    if !entities[ti].alive {
        return;
    }
    entities[ti].hp = (entities[ti].hp - amount).max(0.0);
    events.push(SimEvent::Damage {
        source,
        target,
        amount,
        ability: ability.map(|s| s.to_string()),
    });
    if entities[ti].hp <= 0.0 {
        entities[ti].alive = false;
        let victim_name = entities[ti].name.clone();
        let kind = entities[ti].kind;
        let xp = entities[ti].xp_value;
        let x = entities[ti].x;
        let z = entities[ti].z;
        events.push(SimEvent::Kill {
            killer: source,
            victim: target,
            victim_name,
        });
        if kind == EntityKind::Mob {
            // XP / loot handled by caller with rng after kill detection.
            let _ = (xp, x, z);
        }
    }
}

pub struct KillReward {
    pub victim: EntityId,
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
                        x: e.x,
                        z: e.z,
                        xp: if e.xp_value == 0 { WOLF_XP } else { e.xp_value },
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
        player.hp_max = warrior_hp(player.level);
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

pub fn spawn_wolf_loot(
    next_id: &mut EntityId,
    entities: &mut Vec<Entity>,
    rng: &mut Rng,
    x: f32,
    z: f32,
) -> EntityId {
    let copper = rng.gen_range_u32(WOLF_COPPER_MIN, WOLF_COPPER_MAX);
    let item = if rng.next_f32() < 0.35 {
        Some("Torn Wolf Pelt".to_string())
    } else {
        None
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
    bag_item: &mut Option<String>,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    let loot_ids: Vec<EntityId> = entities
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            *i != pi && e.kind == EntityKind::Loot && e.alive && dist2d(&entities[pi], e) < 2.0
        })
        .map(|(_, e)| e.id)
        .collect();
    for lid in loot_ids {
        let Some(li) = entities.iter().position(|e| e.id == lid) else {
            continue;
        };
        let c = entities[li].loot_copper;
        let item = entities[li].loot_item.clone();
        entities[li].alive = false;
        *copper = copper.saturating_add(c);
        if let Some(ref it) = item {
            *bag_item = Some(it.clone());
        }
        events.push(SimEvent::Loot {
            player: player_id,
            copper: c,
            item,
        });
    }
}

pub fn update_player_combat(
    player_id: EntityId,
    entities: &mut [Entity],
    ability: Option<AbilitySlot>,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    if !entities[pi].alive {
        return;
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

    let d = dist2d(&entities[pi], &entities[ti]);
    if d > MELEE_RANGE {
        return;
    }

    // Face target while swinging.
    let yaw = face_toward(&entities[pi], &entities[ti]);
    entities[pi].yaw = yaw;

    // Heroic Strike: next melee swing deals bonus damage + spends rage.
    let mut heroic = false;
    if matches!(ability, Some(AbilitySlot::Primary))
        && entities[pi].ability_cd <= 0.0
        && entities[pi].resource + 1e-3 >= HEROIC_STRIKE_COST
        && spend_rage(&mut entities[pi], HEROIC_STRIKE_COST)
    {
        heroic = true;
        entities[pi].ability_cd = HEROIC_STRIKE_CD;
        entities[pi].auto_attack = true;
    }

    if !entities[pi].auto_attack && !heroic {
        return;
    }

    entities[pi].swing_timer -= DT;
    if entities[pi].swing_timer > 0.0 && !heroic {
        return;
    }
    entities[pi].swing_timer = WARRIOR_SWING_SEC;

    let mut dmg = WARRIOR_WEAPON_DAMAGE;
    let ability_name = if heroic {
        dmg += HEROIC_STRIKE_BONUS;
        Some("Heroic Strike")
    } else {
        None
    };
    gain_rage(&mut entities[pi], 5.0);
    let src = entities[pi].id;
    deal_damage(entities, src, tid, dmg, ability_name, events);
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
    entities[mi].swing_timer = WOLF_SWING_SEC;
    let src = entities[mi].id;
    deal_damage(entities, src, player_id, WOLF_DAMAGE, None, events);
}
