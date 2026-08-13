//! Combat: auto-attack, primary ability, GCD, casts, auras, damage, death, XP, loot.

use crate::entity::{AuraInstance, CastState, Entity};
use crate::rng::Rng;
use crate::types::{
    player_hp, xp_to_next, MELEE_RANGE, MOB_SWING_SEC, PLAYER_SWING_SEC, RANGED_FALLBACK,
};
use woc_content::{ability, class_ability_for_slot, class_def, mob, AbilityDef, ResourceType};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, SimEvent, DT};

/// Global cooldown after starting an ability (seconds).
pub const GCD_SEC: f32 = 1.5;

/// Default DoT applied by melee kit hits (Rend).
const REND_ID: &str = "rend";
const REND_DURATION: f32 = 9.0;
const REND_TICK_INTERVAL: f32 = 3.0;
const REND_TICK_DAMAGE: f32 = 4.0;

/// DoT applied when Fireball / incinerate lands (Ignite).
const IGNITE_ID: &str = "ignite";
const IGNITE_DURATION: f32 = 8.0;
const IGNITE_TICK_INTERVAL: f32 = 2.0;
const IGNITE_TICK_DAMAGE: f32 = 6.0;

/// Nature / shadow DoT from sting / corruption / moonfire / holy fire.
const STING_ID: &str = "sting";
const STING_DURATION: f32 = 12.0;
const STING_TICK_INTERVAL: f32 = 3.0;
const STING_TICK_DAMAGE: f32 = 5.0;

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

pub fn add_threat(mob: &mut Entity, source: EntityId, amount: f32) {
    if amount <= 0.0 || mob.kind != EntityKind::Mob {
        return;
    }
    *mob.threat.entry(source).or_insert(0.0) += amount;
}

/// Prefer current living target; else highest threat in range; else `None`.
pub fn prefer_mob_target(mob: &Entity, entities: &[Entity], max_range: f32) -> Option<EntityId> {
    if let Some(tid) = mob.target {
        if entities
            .iter()
            .any(|e| e.id == tid && e.alive && e.kind == EntityKind::Player)
        {
            return Some(tid);
        }
    }
    let mut best: Option<(EntityId, f32)> = None;
    for (&id, &threat) in &mob.threat {
        let Some(e) = entities.iter().find(|e| e.id == id) else {
            continue;
        };
        if !e.alive || e.kind != EntityKind::Player {
            continue;
        }
        let d = dist2d(mob, e);
        if d > max_range {
            continue;
        }
        if best.map(|(_, t)| threat > t).unwrap_or(true) {
            best = Some((id, threat));
        }
    }
    best.map(|(id, _)| id)
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
    let talent_mult = entities
        .iter()
        .find(|e| e.id == source)
        .map(crate::talents::damage_multiplier)
        .unwrap_or(1.0);
    let mitigated = (amount * talent_mult - entities[ti].armor * 0.05).max(1.0);
    entities[ti].hp = (entities[ti].hp - mitigated).max(0.0);
    if entities[ti].kind == EntityKind::Mob {
        add_threat(&mut entities[ti], source, mitigated);
    }
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

pub fn apply_aura(target: &mut Entity, aura: AuraInstance, events: &mut Vec<SimEvent>) {
    let id = aura.id.clone();
    let remaining = aura.remaining;
    let stacks = aura.stacks;
    if let Some(existing) = target.auras.iter_mut().find(|a| a.id == aura.id) {
        existing.remaining = existing.remaining.max(aura.remaining);
        existing.stacks = existing.stacks.max(aura.stacks);
        existing.tick_damage = aura.tick_damage;
        existing.tick_heal = aura.tick_heal;
        existing.tick_interval = aura.tick_interval;
        existing.source = aura.source;
    } else {
        target.auras.push(aura);
    }
    events.push(SimEvent::AuraApplied {
        player: target.id,
        id,
        remaining,
        stacks,
    });
}

fn apply_primary_dot(
    entities: &mut [Entity],
    source: EntityId,
    target: EntityId,
    ability_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(ti) = entities.iter().position(|e| e.id == target) else {
        return;
    };
    if !entities[ti].alive {
        return;
    }
    let aura = match ability_id {
        "fireball" | "incinerate" | "lava_burst" => AuraInstance {
            id: IGNITE_ID.into(),
            remaining: IGNITE_DURATION,
            stacks: 1,
            tick_timer: IGNITE_TICK_INTERVAL,
            tick_interval: IGNITE_TICK_INTERVAL,
            tick_damage: IGNITE_TICK_DAMAGE,
            tick_heal: 0.0,
            source,
        },
        "heroic_strike" | "cleave" | "crusader_strike" | "sinister_strike" | "eviscerate" => {
            AuraInstance {
                id: REND_ID.into(),
                remaining: REND_DURATION,
                stacks: 1,
                tick_timer: REND_TICK_INTERVAL,
                tick_interval: REND_TICK_INTERVAL,
                tick_damage: REND_TICK_DAMAGE,
                tick_heal: 0.0,
                source,
            }
        }
        "serpent_sting" | "corruption" | "moonfire" | "holy_fire" => AuraInstance {
            id: STING_ID.into(),
            remaining: STING_DURATION,
            stacks: 1,
            tick_timer: STING_TICK_INTERVAL,
            tick_interval: STING_TICK_INTERVAL,
            tick_damage: STING_TICK_DAMAGE,
            tick_heal: 0.0,
            source,
        },
        _ => return,
    };
    apply_aura(&mut entities[ti], aura, events);
}

/// Tick all entity auras: DoT damage, HoT heals, and expiry.
pub fn tick_auras(entities: &mut [Entity], events: &mut Vec<SimEvent>) {
    // Collect tick applications first to avoid borrow issues across entities.
    let mut pending_dots: Vec<(EntityId, EntityId, f32, String)> = Vec::new();
    let mut pending_hots: Vec<(EntityId, f32, String)> = Vec::new();
    let ids: Vec<EntityId> = entities.iter().map(|e| e.id).collect();

    for id in ids {
        let Some(ei) = entities.iter().position(|e| e.id == id) else {
            continue;
        };
        if !entities[ei].alive && entities[ei].kind != EntityKind::Mob {
            entities[ei].auras.clear();
            continue;
        }
        let mut expired = Vec::new();
        for (ai, aura) in entities[ei].auras.iter_mut().enumerate() {
            aura.remaining -= DT;
            let has_tick =
                (aura.tick_damage > 0.0 || aura.tick_heal > 0.0) && aura.tick_interval > 0.0;
            if has_tick {
                aura.tick_timer -= DT;
                if aura.tick_timer <= 0.0 {
                    aura.tick_timer += aura.tick_interval;
                    let stacks = aura.stacks.max(1) as f32;
                    if aura.tick_damage > 0.0 {
                        pending_dots.push((
                            aura.source,
                            id,
                            aura.tick_damage * stacks,
                            aura.id.clone(),
                        ));
                    }
                    if aura.tick_heal > 0.0 {
                        pending_hots.push((id, aura.tick_heal * stacks, aura.id.clone()));
                    }
                }
            }
            if aura.remaining <= 0.0 {
                expired.push(ai);
            }
        }
        for ai in expired.into_iter().rev() {
            entities[ei].auras.remove(ai);
        }
    }

    for (source, target, amount, aura_id) in pending_dots {
        deal_damage(entities, source, target, amount, Some(&aura_id), events);
    }
    for (target, amount, aura_id) in pending_hots {
        apply_hot_tick(entities, target, amount, &aura_id, events);
    }
}

fn apply_hot_tick(
    entities: &mut [Entity],
    target: EntityId,
    amount: f32,
    aura_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(ti) = entities.iter().position(|e| e.id == target) else {
        return;
    };
    if !entities[ti].alive {
        return;
    }
    let before = entities[ti].hp;
    entities[ti].hp = (entities[ti].hp + amount).min(entities[ti].hp_max);
    let healed = entities[ti].hp - before;
    if healed > 0.0 {
        events.push(SimEvent::Toast {
            message: format!("{aura_id} heals for {:.0}.", healed),
        });
    }
}

pub struct KillReward {
    pub killer: EntityId,
    pub victim: EntityId,
    pub template_id: Option<String>,
    pub x: f32,
    pub z: f32,
    pub xp: u32,
}

pub fn collect_pending_mob_kills(events: &[SimEvent], entities: &[Entity]) -> Vec<KillReward> {
    let mut out = Vec::new();
    for ev in events {
        if let SimEvent::Kill { killer, victim, .. } = ev {
            if let Some(e) = entities.iter().find(|e| e.id == *victim) {
                if e.kind == EntityKind::Mob {
                    out.push(KillReward {
                        killer: *killer,
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

pub fn grant_xp(player: &mut Entity, amount: u32, events: &mut Vec<SimEvent>) {
    player.xp = player.xp.saturating_add(amount);
    loop {
        let need = xp_to_next(player.level);
        if player.xp < need {
            break;
        }
        player.xp -= need;
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
        crate::talents::on_level_up(player);
        crate::entity::refresh_known_abilities(player);
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
    entities.push(crate::entity::create_loot(id, x, z, copper, item));
    id
}

pub fn try_pickup_loot(
    player_id: EntityId,
    entities: &mut [Entity],
    events: &mut Vec<SimEvent>,
    pending: &crate::social::LootRules,
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
                && !pending.is_pending(e.id)
                // Profession gather nodes are harvested via Interact, not auto-loot.
                && e.template_id
                    .as_deref()
                    .and_then(woc_content::gather_node)
                    .is_none()
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
        entities[pi].copper = entities[pi].copper.saturating_add(c);
        events.push(SimEvent::Loot {
            player: player_id,
            copper: c,
            item,
        });
    }
}

/// Claim a specific loot pile (or loot near a corpse) via Interact.
pub fn claim_loot_target(
    player_id: EntityId,
    target_id: EntityId,
    entities: &mut [Entity],
    events: &mut Vec<SimEvent>,
    pending: &crate::social::LootRules,
) -> bool {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return false;
    };
    if !entities[pi].alive {
        return false;
    }

    // Direct loot pile.
    if let Some(li) = entities
        .iter()
        .position(|e| e.id == target_id && e.kind == EntityKind::Loot && e.alive)
    {
        if dist2d(&entities[pi], &entities[li]) > crate::types::INTERACT_RANGE {
            events.push(SimEvent::Toast {
                message: "Too far to loot.".into(),
            });
            return false;
        }
        if pending.is_pending(target_id) {
            events.push(SimEvent::Toast {
                message: "Need/Greed roll in progress (1 Need / 2 Greed / 3 Pass).".into(),
            });
            return false;
        }
        if entities[li]
            .template_id
            .as_deref()
            .and_then(woc_content::gather_node)
            .is_some()
        {
            return false;
        }
        let c = entities[li].loot_copper;
        let item = entities[li].loot_item.clone();
        if let Some(ref it) = item {
            if crate::inventory::grant_item(&mut entities[pi], it, 1, events).is_err() {
                events.push(SimEvent::Toast {
                    message: "Inventory full.".into(),
                });
                return false;
            }
            crate::quests::on_inventory_changed(&mut entities[pi], events);
        }
        entities[li].alive = false;
        entities[pi].copper = entities[pi].copper.saturating_add(c);
        events.push(SimEvent::Loot {
            player: player_id,
            copper: c,
            item,
        });
        return true;
    }

    // Dead mob corpse: vacuum nearby loot piles.
    let Some(ci) = entities
        .iter()
        .position(|e| e.id == target_id && e.kind == EntityKind::Mob && !e.alive)
    else {
        return false;
    };
    if dist2d(&entities[pi], &entities[ci]) > crate::types::INTERACT_RANGE {
        events.push(SimEvent::Toast {
            message: "Too far to loot.".into(),
        });
        return false;
    }
    let cx = entities[ci].x;
    let cz = entities[ci].z;
    let loot_ids: Vec<EntityId> = entities
        .iter()
        .filter(|e| {
            e.kind == EntityKind::Loot
                && e.alive
                && !pending.is_pending(e.id)
                && e.template_id
                    .as_deref()
                    .and_then(woc_content::gather_node)
                    .is_none()
                && {
                    let dx = e.x - cx;
                    let dz = e.z - cz;
                    (dx * dx + dz * dz).sqrt() < crate::types::LOOT_RANGE
                }
        })
        .map(|e| e.id)
        .collect();
    if loot_ids.is_empty() {
        // Pending rolls near corpse?
        let pending_near = entities.iter().any(|e| {
            e.kind == EntityKind::Loot && e.alive && pending.is_pending(e.id) && {
                let dx = e.x - cx;
                let dz = e.z - cz;
                (dx * dx + dz * dz).sqrt() < crate::types::LOOT_RANGE
            }
        });
        if pending_near {
            events.push(SimEvent::Toast {
                message: "Need/Greed roll in progress (1 Need / 2 Greed / 3 Pass).".into(),
            });
        } else {
            events.push(SimEvent::Toast {
                message: "Nothing to loot.".into(),
            });
        }
        return false;
    }
    let before = events.len();
    for lid in loot_ids {
        // Reuse pickup logic by temporarily placing player next to pile distance-wise
        // (already verified range via corpse). Call through a one-shot filter.
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
        entities[pi].copper = entities[pi].copper.saturating_add(c);
        events.push(SimEvent::Loot {
            player: player_id,
            copper: c,
            item,
        });
    }
    events.len() > before
}

fn ability_range(player: &Entity) -> f32 {
    player
        .primary_ability
        .as_deref()
        .and_then(ability)
        .map(|a| a.range)
        .unwrap_or(MELEE_RANGE)
}

fn slot_as_u8(slot: AbilitySlot) -> u8 {
    match slot {
        AbilitySlot::Primary => 1,
        AbilitySlot::Slot2 => 2,
        AbilitySlot::Slot3 => 3,
        AbilitySlot::Slot4 => 4,
        AbilitySlot::Slot5 => 5,
    }
}

/// Resolve a pressed ability slot to a known, level-unlocked ability def.
fn resolve_slot_ability(player: &Entity, slot: AbilitySlot) -> Option<&'static AbilityDef> {
    let class = player.class_id?;
    let def = class_ability_for_slot(class, slot_as_u8(slot))?;
    player
        .known_abilities
        .iter()
        .any(|id| id == def.id)
        .then_some(def)
}

fn tick_ability_cds(player: &mut Entity) {
    let ids: Vec<String> = player.ability_cds.keys().cloned().collect();
    for id in ids {
        if let Some(cd) = player.ability_cds.get_mut(&id) {
            *cd = (*cd - DT).max(0.0);
        }
    }
    player.ability_cds.retain(|_, cd| *cd > 0.0);
    // Keep legacy `ability_cd` mirrored to the primary for HUD/snapshot.
    player.ability_cd = player
        .primary_ability
        .as_deref()
        .and_then(|id| player.ability_cds.get(id).copied())
        .unwrap_or(0.0);
}

fn start_ability_cd(player: &mut Entity, abil_id: &str, cooldown: f32) {
    player.ability_cds.insert(abil_id.to_string(), cooldown);
    if player.primary_ability.as_deref() == Some(abil_id) {
        player.ability_cd = cooldown;
    }
}

fn ability_on_cd(player: &Entity, abil_id: &str) -> bool {
    player.ability_cds.get(abil_id).copied().unwrap_or(0.0) > 0.0
}

fn resolve_ability_hit(
    entities: &mut [Entity],
    src: EntityId,
    tid: EntityId,
    abil_id: &str,
    def_name: &str,
    def_damage: f32,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == src) else {
        return;
    };
    let dmg = def_damage + entities[pi].attack_damage * 0.35;
    if matches!(entities[pi].resource_type, Some(ResourceType::Rage)) {
        gain_resource(&mut entities[pi], 5.0);
    }
    deal_damage(entities, src, tid, dmg, Some(def_name), events);
    apply_primary_dot(entities, src, tid, abil_id, events);
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
        entities[pi].cast = None;
        return;
    }

    // Soft regen for mana/energy out of swings.
    if let Some(ResourceType::Mana | ResourceType::Energy) = entities[pi].resource_type {
        gain_resource(&mut entities[pi], 1.5 * DT);
    }

    tick_ability_cds(&mut entities[pi]);
    if entities[pi].gcd > 0.0 {
        entities[pi].gcd = (entities[pi].gcd - DT).max(0.0);
    }

    let target_id = entities[pi].target;
    let Some(tid) = target_id else {
        entities[pi].swing_timer = 0.0;
        entities[pi].cast = None;
        return;
    };
    let Some(ti) = entities.iter().position(|e| e.id == tid) else {
        entities[pi].target = None;
        entities[pi].cast = None;
        return;
    };
    if !entities[ti].alive || entities[ti].kind != EntityKind::Mob {
        entities[pi].target = None;
        entities[pi].auto_attack = false;
        entities[pi].cast = None;
        return;
    }

    let range = ability_range(&entities[pi]).max(MELEE_RANGE);
    let d = dist2d(&entities[pi], &entities[ti]);
    let in_melee = d <= MELEE_RANGE;

    entities[pi].yaw = face_toward(&entities[pi], &entities[ti]);

    // Advance in-progress cast.
    if entities[pi].cast.is_some() {
        let cast_range = entities[pi]
            .cast
            .as_ref()
            .and_then(|c| ability(&c.ability_id))
            .map(|a| a.range)
            .unwrap_or(range);
        let in_cast_range = d <= cast_range.max(RANGED_FALLBACK.min(cast_range));
        let cast_target = entities[pi].cast.as_ref().map(|c| c.target);
        if cast_target != Some(tid) || !in_cast_range {
            entities[pi].cast = None;
        } else if let Some(mut cast) = entities[pi].cast.take() {
            cast.elapsed += DT;
            if cast.elapsed >= cast.duration {
                let abil_id = cast.ability_id.clone();
                if let Some(def) = ability(&abil_id) {
                    let src = entities[pi].id;
                    resolve_ability_hit(entities, src, tid, &abil_id, def.name, def.damage, events);
                }
            } else {
                entities[pi].cast = Some(cast);
            }
        }
        // While casting, still allow auto-attack below; do not start a new ability.
    } else if let Some(slot) = ability_slot {
        if entities[pi].gcd <= 0.0 {
            if let Some(def) = resolve_slot_ability(&entities[pi], slot) {
                let abil_id = def.id;
                let abil_range = def.range.max(RANGED_FALLBACK.min(def.range));
                let in_slot_range = d <= abil_range;
                if in_slot_range
                    && !ability_on_cd(&entities[pi], abil_id)
                    && spend_resource(&mut entities[pi], def.cost)
                {
                    start_ability_cd(&mut entities[pi], abil_id, def.cooldown);
                    entities[pi].gcd = GCD_SEC;
                    entities[pi].auto_attack = true;
                    if def.cast_time > 0.0 {
                        entities[pi].cast = Some(CastState {
                            ability_id: abil_id.to_string(),
                            elapsed: 0.0,
                            duration: def.cast_time,
                            target: tid,
                        });
                    } else {
                        let src = entities[pi].id;
                        resolve_ability_hit(
                            entities, src, tid, abil_id, def.name, def.damage, events,
                        );
                        // Instant ability: skip auto this frame (legacy behavior).
                        return;
                    }
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

    // Prefer sticky current target / threat table over the suggested focus.
    let focus = prefer_mob_target(&entities[mi], entities, 40.0).unwrap_or(player_id);
    entities[mi].target = Some(focus);

    let Some(pi) = entities.iter().position(|e| e.id == focus) else {
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
    deal_damage(entities, src, focus, dmg, None, events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::AuraInstance;
    use woc_content::PlayerClass;
    use woc_protocol::AbilitySlot;

    fn player_and_mob() -> (Entity, Entity) {
        let mut player = crate::entity::create_player(1, "Tester", PlayerClass::Warrior, 0.0, 0.0);
        player.resource = 100.0;
        player.ability_cd = 0.0;
        player.gcd = 0.0;
        player.primary_ability = Some("heroic_strike".into());
        let mut mob =
            crate::entity::create_mob_from_template(2, "young_wolf", 1.0, 0.0).expect("wolf");
        mob.hp = 500.0;
        mob.hp_max = 500.0;
        player.target = Some(mob.id);
        player.auto_attack = true;
        (player, mob)
    }

    #[test]
    fn gcd_blocks_second_ability_cast() {
        let (player, mob) = player_and_mob();
        let mob_hp = mob.hp;
        let mut entities = vec![player, mob];
        let mut events = Vec::new();

        update_player_combat(1, &mut entities, Some(AbilitySlot::Primary), &mut events);
        let after_first = entities[1].hp;
        assert!(after_first < mob_hp, "first cast should deal damage");
        assert!(entities[0].gcd > 0.0, "GCD should start after ability");

        // Clear per-ability CD so only GCD can block. Freeze auto-swing so it
        // cannot masquerade as an ability hit.
        entities[0].ability_cd = 0.0;
        entities[0].ability_cds.clear();
        entities[0].resource = 100.0;
        entities[0].auto_attack = false;
        entities[0].swing_timer = 99.0;
        let hp_before_second = entities[1].hp;
        let auras_before = entities[1].auras.len();
        events.clear();
        update_player_combat(1, &mut entities, Some(AbilitySlot::Primary), &mut events);
        assert_eq!(
            entities[1].hp, hp_before_second,
            "GCD must block a second ability cast"
        );
        assert_eq!(
            entities[1].auras.len(),
            auras_before,
            "GCD must not re-apply primary DoT"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                SimEvent::Damage {
                    ability: Some(_),
                    ..
                }
            )),
            "no ability damage while on GCD"
        );

        // Expire GCD and ability CD; cast should land again.
        entities[0].gcd = 0.0;
        entities[0].ability_cd = 0.0;
        entities[0].ability_cds.clear();
        entities[0].resource = 100.0;
        entities[0].auto_attack = false;
        update_player_combat(1, &mut entities, Some(AbilitySlot::Primary), &mut events);
        assert!(
            entities[1].hp < hp_before_second,
            "cast after GCD should hit"
        );
    }

    #[test]
    fn aura_expires_after_remaining_elapses() {
        let mut mob = crate::entity::create_mob_from_template(2, "young_wolf", 0.0, 0.0).unwrap();
        mob.auras.push(AuraInstance {
            id: "rend".into(),
            remaining: DT * 1.5,
            stacks: 1,
            tick_timer: 999.0,
            tick_interval: 999.0,
            tick_damage: 0.0,
            tick_heal: 0.0,
            source: 1,
        });
        let mut entities = vec![mob];
        let mut events = Vec::new();

        tick_auras(&mut entities, &mut events);
        assert_eq!(entities[0].auras.len(), 1, "aura still active mid-duration");
        assert!(entities[0].auras[0].remaining > 0.0);

        tick_auras(&mut entities, &mut events);
        assert!(
            entities[0].auras.is_empty(),
            "aura must expire once remaining elapses"
        );
    }

    #[test]
    fn aura_dot_ticks_damage_each_interval() {
        let mut mob = crate::entity::create_mob_from_template(2, "young_wolf", 0.0, 0.0).unwrap();
        let start_hp = mob.hp;
        mob.auras.push(AuraInstance {
            id: "rend".into(),
            remaining: 10.0,
            stacks: 1,
            tick_timer: DT,
            tick_interval: 3.0 * DT,
            tick_damage: 7.0,
            tick_heal: 0.0,
            source: 1,
        });
        let mut entities = vec![mob];
        let mut events = Vec::new();

        tick_auras(&mut entities, &mut events);
        assert!(entities[0].hp < start_hp, "DoT should tick once");
        let after_tick = entities[0].hp;

        tick_auras(&mut entities, &mut events);
        assert_eq!(
            entities[0].hp, after_tick,
            "no second tick before interval elapses"
        );
    }

    #[test]
    fn fireball_starts_timed_cast() {
        let mut player = crate::entity::create_player(1, "Mage", PlayerClass::Mage, 0.0, 0.0);
        player.resource = 100.0;
        player.primary_ability = Some("fireball".into());
        let mut mob = crate::entity::create_mob_from_template(2, "young_wolf", 5.0, 0.0).unwrap();
        mob.hp = 500.0;
        mob.hp_max = 500.0;
        player.target = Some(mob.id);
        let start_hp = mob.hp;
        let mut entities = vec![player, mob];
        let mut events = Vec::new();

        update_player_combat(1, &mut entities, Some(AbilitySlot::Primary), &mut events);
        assert!(entities[0].cast.is_some(), "fireball should begin casting");
        assert_eq!(entities[1].hp, start_hp, "no damage until cast completes");
        assert!(entities[0].gcd > 0.0);

        // Advance cast to completion.
        let duration = entities[0].cast.as_ref().unwrap().duration;
        let ticks = (duration / DT).ceil() as u32 + 1;
        for _ in 0..ticks {
            update_player_combat(1, &mut entities, None, &mut events);
        }
        assert!(entities[0].cast.is_none());
        assert!(entities[1].hp < start_hp, "damage after cast finishes");
        assert!(
            entities[1].auras.iter().any(|a| a.id == IGNITE_ID),
            "ignite DoT applied on fireball hit"
        );
    }

    #[test]
    fn create_player_knows_level_one_kit_abilities() {
        let player = crate::entity::create_player(1, "W", PlayerClass::Warrior, 0.0, 0.0);
        assert!(
            player.known_abilities.iter().any(|a| a == "heroic_strike"),
            "primary known at level 1"
        );
        assert!(
            !player.known_abilities.iter().any(|a| a == "cleave"),
            "slot2 gated until level 3"
        );
        assert!(
            !player.known_abilities.iter().any(|a| a == "execute"),
            "slot3 gated until level 6"
        );
    }

    #[test]
    fn level_up_unlocks_gated_kit_abilities() {
        let mut player = crate::entity::create_player(1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        grant_xp(&mut player, 10_000, &mut events);
        assert!(player.level >= 3, "leveled enough for cleave");
        assert!(
            player.known_abilities.iter().any(|a| a == "cleave"),
            "cleave unlocked after level gate"
        );
    }

    #[test]
    fn slot2_ability_deals_damage_when_known() {
        let mut player = crate::entity::create_player(1, "W", PlayerClass::Warrior, 0.0, 0.0);
        player.level = 3;
        crate::entity::refresh_known_abilities(&mut player);
        player.resource = 100.0;
        player.ability_cd = 0.0;
        player.gcd = 0.0;
        let mut mob =
            crate::entity::create_mob_from_template(2, "young_wolf", 1.0, 0.0).expect("wolf");
        mob.hp = 500.0;
        mob.hp_max = 500.0;
        player.target = Some(mob.id);
        let start_hp = mob.hp;
        let mut entities = vec![player, mob];
        let mut events = Vec::new();

        update_player_combat(1, &mut entities, Some(AbilitySlot::Slot2), &mut events);
        assert!(
            entities[1].hp < start_hp,
            "slot2 (cleave) should deal damage when known"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::Damage { ability: Some(n), .. } if n == "Cleave")),
            "cleave damage event"
        );
    }

    #[test]
    fn slot2_blocked_when_ability_unknown() {
        let mut player = crate::entity::create_player(1, "W", PlayerClass::Warrior, 0.0, 0.0);
        assert_eq!(player.level, 1);
        player.resource = 100.0;
        let mut mob =
            crate::entity::create_mob_from_template(2, "young_wolf", 1.0, 0.0).expect("wolf");
        mob.hp = 500.0;
        mob.hp_max = 500.0;
        player.target = Some(mob.id);
        let start_hp = mob.hp;
        let mut entities = vec![player, mob];
        let mut events = Vec::new();

        update_player_combat(1, &mut entities, Some(AbilitySlot::Slot2), &mut events);
        assert_eq!(
            entities[1].hp, start_hp,
            "unknown slot2 must not deal damage"
        );
    }

    #[test]
    fn slot3_and_primary_use_independent_cooldowns() {
        let mut player = crate::entity::create_player(1, "W", PlayerClass::Warrior, 0.0, 0.0);
        player.level = 6;
        crate::entity::refresh_known_abilities(&mut player);
        player.resource = 100.0;
        let mut mob =
            crate::entity::create_mob_from_template(2, "young_wolf", 1.0, 0.0).expect("wolf");
        mob.hp = 500.0;
        mob.hp_max = 500.0;
        player.target = Some(mob.id);
        let mut entities = vec![player, mob];
        let mut events = Vec::new();

        update_player_combat(1, &mut entities, Some(AbilitySlot::Primary), &mut events);
        let after_primary = entities[1].hp;
        assert!(after_primary < 500.0);

        // Clear GCD only; primary CD remains. Slot3 should still fire.
        entities[0].gcd = 0.0;
        entities[0].resource = 100.0;
        entities[0].auto_attack = false;
        entities[0].swing_timer = 99.0;
        events.clear();
        update_player_combat(1, &mut entities, Some(AbilitySlot::Slot3), &mut events);
        assert!(
            entities[1].hp < after_primary,
            "slot3 execute should ignore primary ability CD"
        );
    }
}
