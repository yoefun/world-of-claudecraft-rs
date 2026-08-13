//! Combat: auto-attack, primary ability, GCD, casts, auras, damage, death, XP, loot.

use crate::ecs::components::{
    Auras, ClassKit, Combat, Health, Identity, LootTable, Progress, Threat, Transform,
};
use crate::ecs::World;
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

fn dist2d_ids(world: &World, a: EntityId, b: EntityId) -> f32 {
    crate::ecs::components::dist2d(world, a, b).unwrap_or(f32::MAX)
}

fn face_toward_ids(world: &World, from: EntityId, to: EntityId) -> f32 {
    let Some(a) = world.get::<Transform>(from) else {
        return 0.0;
    };
    let Some(b) = world.get::<Transform>(to) else {
        return 0.0;
    };
    (b.x - a.x).atan2(b.z - a.z)
}

fn gain_resource(kit: &mut ClassKit, amount: f32) {
    kit.resource = (kit.resource + amount).min(kit.resource_max);
}

fn spend_resource(kit: &mut ClassKit, amount: f32) -> bool {
    if kit.resource + 1e-3 < amount {
        return false;
    }
    kit.resource -= amount;
    true
}

pub fn add_threat(world: &mut World, mob_id: EntityId, source: EntityId, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let Some(threat) = world.get_mut::<Threat>(mob_id) else {
        return;
    };
    *threat.threat.entry(source).or_insert(0.0) += amount;
}

/// Prefer current living target; else highest threat in range; else `None`.
pub fn prefer_mob_target(world: &World, mob_id: EntityId, max_range: f32) -> Option<EntityId> {
    if let Some(tid) = world.get::<Combat>(mob_id).and_then(|c| c.target) {
        if world.get::<ClassKit>(tid).is_some()
            && world.get::<Health>(tid).is_some_and(|h| h.alive)
        {
            return Some(tid);
        }
    }
    let threat = world.get::<Threat>(mob_id)?.threat.clone();
    let mut best: Option<(EntityId, f32)> = None;
    for (id, threat_val) in threat {
        if world.get::<ClassKit>(id).is_none() {
            continue;
        }
        if !world.get::<Health>(id).is_some_and(|h| h.alive) {
            continue;
        }
        let d = dist2d_ids(world, mob_id, id);
        if d > max_range {
            continue;
        }
        if best.map(|(_, t)| threat_val > t).unwrap_or(true) {
            best = Some((id, threat_val));
        }
    }
    best.map(|(id, _)| id)
}

pub fn deal_damage(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    amount: f32,
    ability_name: Option<&str>,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Health>(target).is_none_or(|h| !h.alive) {
        return;
    }
    if world.get::<Identity>(target).is_some_and(|i| i.kind == EntityKind::Npc) {
        return;
    }
    let talent_mult = world
        .get::<Progress>(source)
        .map(|p| crate::talents::damage_multiplier_from_ranks(&p.talents))
        .unwrap_or(1.0);
    let armor = world.get::<Combat>(target).map(|c| c.armor).unwrap_or(0.0);
    let mitigated = (amount * talent_mult - armor * 0.05).max(1.0);
    let Some(health) = world.get_mut::<Health>(target) else {
        return;
    };
    health.hp = (health.hp - mitigated).max(0.0);
    let died = health.hp <= 0.0;
    if died {
        health.alive = false;
    }
    add_threat(world, target, source, mitigated);
    events.push(SimEvent::Damage {
        source,
        target,
        amount: mitigated,
        ability: ability_name.map(|s| s.to_string()),
    });
    if died {
        let victim_name = world
            .get::<Identity>(target)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        events.push(SimEvent::Kill {
            killer: source,
            victim: target,
            victim_name,
        });
    }
}

/// Bridge for modules still on `&mut [Entity]` (pets).
pub fn deal_damage_entities(
    entities: &mut [Entity],
    source: EntityId,
    target: EntityId,
    amount: f32,
    ability_name: Option<&str>,
    events: &mut Vec<SimEvent>,
) {
    let mut world = World::new();
    for e in entities.iter() {
        crate::ecs::spawn::sync_entity_to_world(&mut world, e);
    }
    deal_damage(&mut world, source, target, amount, ability_name, events);
    crate::ecs::spawn::apply_world_to_entities(&world, entities);
}

pub fn apply_aura(world: &mut World, target: EntityId, aura: AuraInstance, events: &mut Vec<SimEvent>) {
    let id = aura.id.clone();
    let remaining = aura.remaining;
    let stacks = aura.stacks;
    let auras = world.get_mut::<Auras>(target);
    if let Some(store) = auras {
        if let Some(existing) = store.auras.iter_mut().find(|a| a.id == aura.id) {
            existing.remaining = existing.remaining.max(aura.remaining);
            existing.stacks = existing.stacks.max(aura.stacks);
            existing.tick_damage = aura.tick_damage;
            existing.tick_heal = aura.tick_heal;
            existing.tick_interval = aura.tick_interval;
            existing.source = aura.source;
        } else {
            store.auras.push(aura);
        }
    } else {
        world.insert(
            target,
            Auras {
                auras: vec![aura],
            },
        );
    }
    events.push(SimEvent::AuraApplied {
        player: target,
        id,
        remaining,
        stacks,
    });
}

fn apply_primary_dot(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    ability_id: &str,
    events: &mut Vec<SimEvent>,
) {
    if !world.get::<Health>(target).is_some_and(|h| h.alive) {
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
    apply_aura(world, target, aura, events);
}

/// Tick all entity auras: DoT damage, HoT heals, and expiry.
pub fn tick_auras(world: &mut World, events: &mut Vec<SimEvent>) {
    let mut pending_dots: Vec<(EntityId, EntityId, f32, String)> = Vec::new();
    let mut pending_hots: Vec<(EntityId, f32, String)> = Vec::new();
    let ids = world.ids::<Auras>();

    for id in ids {
        let alive = world.get::<Health>(id).is_some_and(|h| h.alive);
        let is_mob = world.get::<LootTable>(id).is_some();
        if !alive && !is_mob {
            if let Some(store) = world.get_mut::<Auras>(id) {
                store.auras.clear();
            }
            continue;
        }
        let Some(store) = world.get_mut::<Auras>(id) else {
            continue;
        };
        let mut expired = Vec::new();
        for (ai, aura) in store.auras.iter_mut().enumerate() {
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
            store.auras.remove(ai);
        }
    }

    for (source, target, amount, aura_id) in pending_dots {
        deal_damage(world, source, target, amount, Some(&aura_id), events);
    }
    for (target, amount, aura_id) in pending_hots {
        apply_hot_tick(world, target, amount, &aura_id, events);
    }
}

fn apply_hot_tick(
    world: &mut World,
    target: EntityId,
    amount: f32,
    aura_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(health) = world.get_mut::<Health>(target) else {
        return;
    };
    if !health.alive {
        return;
    }
    let before = health.hp;
    health.hp = (health.hp + amount).min(health.hp_max);
    let healed = health.hp - before;
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

pub fn try_pickup_loot(player_id: EntityId, entities: &mut [Entity], events: &mut Vec<SimEvent>) {
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

fn ability_range(kit: &ClassKit) -> f32 {
    kit.primary_ability
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
fn resolve_slot_ability(kit: &ClassKit, slot: AbilitySlot) -> Option<&'static AbilityDef> {
    let class = kit.class_id?;
    let def = class_ability_for_slot(class, slot_as_u8(slot))?;
    kit.known_abilities
        .iter()
        .any(|id| id == def.id)
        .then_some(def)
}

fn tick_ability_cds(kit: &mut ClassKit, combat: &mut Combat) {
    let ids: Vec<String> = kit.ability_cds.keys().cloned().collect();
    for id in ids {
        if let Some(cd) = kit.ability_cds.get_mut(&id) {
            *cd = (*cd - DT).max(0.0);
        }
    }
    kit.ability_cds.retain(|_, cd| *cd > 0.0);
    combat.ability_cd = kit
        .primary_ability
        .as_deref()
        .and_then(|id| kit.ability_cds.get(id).copied())
        .unwrap_or(0.0);
}

fn start_ability_cd(kit: &mut ClassKit, combat: &mut Combat, abil_id: &str, cooldown: f32) {
    kit.ability_cds.insert(abil_id.to_string(), cooldown);
    if kit.primary_ability.as_deref() == Some(abil_id) {
        combat.ability_cd = cooldown;
    }
}

fn ability_on_cd(kit: &ClassKit, abil_id: &str) -> bool {
    kit.ability_cds.get(abil_id).copied().unwrap_or(0.0) > 0.0
}

fn resolve_ability_hit(
    world: &mut World,
    src: EntityId,
    tid: EntityId,
    abil_id: &str,
    def_name: &str,
    def_damage: f32,
    events: &mut Vec<SimEvent>,
) {
    let attack = world.get::<Combat>(src).map(|c| c.attack_damage).unwrap_or(0.0);
    let rage = world
        .get::<ClassKit>(src)
        .and_then(|k| k.resource_type)
        .is_some_and(|rt| matches!(rt, ResourceType::Rage));
    if rage {
        if let Some(kit) = world.get_mut::<ClassKit>(src) {
            gain_resource(kit, 5.0);
        }
    }
    let dmg = def_damage + attack * 0.35;
    deal_damage(world, src, tid, dmg, Some(def_name), events);
    apply_primary_dot(world, src, tid, abil_id, events);
}

fn is_living_mob(world: &World, id: EntityId) -> bool {
    world.get::<LootTable>(id).is_some() && world.get::<Health>(id).is_some_and(|h| h.alive)
}

pub fn update_player_combat(
    player_id: EntityId,
    world: &mut World,
    ability_slot: Option<AbilitySlot>,
    events: &mut Vec<SimEvent>,
) {
    if !world.get::<Health>(player_id).is_some_and(|h| h.alive) {
        if let Some(c) = world.get_mut::<Combat>(player_id) {
            c.cast = None;
        }
        return;
    }
    let Some(mut combat) = world.get::<Combat>(player_id).cloned() else {
        return;
    };
    let Some(mut kit) = world.get::<ClassKit>(player_id).cloned() else {
        return;
    };

    if let Some(ResourceType::Mana | ResourceType::Energy) = kit.resource_type {
        gain_resource(&mut kit, 1.5 * DT);
    }

    tick_ability_cds(&mut kit, &mut combat);
    if combat.gcd > 0.0 {
        combat.gcd = (combat.gcd - DT).max(0.0);
    }

    let target_id = combat.target;
    let Some(tid) = target_id else {
        combat.swing_timer = 0.0;
        combat.cast = None;
        world.insert(player_id, combat);
        world.insert(player_id, kit);
        return;
    };
    if !is_living_mob(world, tid) {
        combat.target = None;
        combat.auto_attack = false;
        combat.cast = None;
        world.insert(player_id, combat);
        world.insert(player_id, kit);
        return;
    }

    let range = ability_range(&kit).max(MELEE_RANGE);
    let d = dist2d_ids(world, player_id, tid);
    let in_melee = d <= MELEE_RANGE;

    let yaw = face_toward_ids(world, player_id, tid);
    if let Some(t) = world.get_mut::<Transform>(player_id) {
        t.yaw = yaw;
    }

    if combat.cast.is_some() {
        let cast_range = combat
            .cast
            .as_ref()
            .and_then(|c| ability(&c.ability_id))
            .map(|a| a.range)
            .unwrap_or(range);
        let in_cast_range = d <= cast_range.max(RANGED_FALLBACK.min(cast_range));
        let cast_target = combat.cast.as_ref().map(|c| c.target);
        if cast_target != Some(tid) || !in_cast_range {
            combat.cast = None;
            world.insert(player_id, combat.clone());
            world.insert(player_id, kit.clone());
        } else if let Some(mut cast) = combat.cast.take() {
            cast.elapsed += DT;
            if cast.elapsed >= cast.duration {
                let abil_id = cast.ability_id.clone();
                world.insert(player_id, combat.clone());
                world.insert(player_id, kit.clone());
                if let Some(def) = ability(&abil_id) {
                    resolve_ability_hit(world, player_id, tid, &abil_id, def.name, def.damage, events);
                }
                combat = world.get::<Combat>(player_id).cloned().unwrap_or(combat);
                kit = world.get::<ClassKit>(player_id).cloned().unwrap_or(kit);
            } else {
                combat.cast = Some(cast);
            }
        }
    } else if let Some(slot) = ability_slot {
        if combat.gcd <= 0.0 {
            if let Some(def) = resolve_slot_ability(&kit, slot) {
                let abil_id = def.id;
                let abil_range = def.range.max(RANGED_FALLBACK.min(def.range));
                let in_slot_range = d <= abil_range;
                if in_slot_range && !ability_on_cd(&kit, abil_id) && spend_resource(&mut kit, def.cost)
                {
                    start_ability_cd(&mut kit, &mut combat, abil_id, def.cooldown);
                    combat.gcd = GCD_SEC;
                    combat.auto_attack = true;
                    if def.cast_time > 0.0 {
                        combat.cast = Some(CastState {
                            ability_id: abil_id.to_string(),
                            elapsed: 0.0,
                            duration: def.cast_time,
                            target: tid,
                        });
                    } else {
                        world.insert(player_id, combat.clone());
                        world.insert(player_id, kit.clone());
                        resolve_ability_hit(
                            world, player_id, tid, abil_id, def.name, def.damage, events,
                        );
                        return;
                    }
                }
            }
        }
    }

    if !combat.auto_attack || !in_melee {
        world.insert(player_id, combat);
        world.insert(player_id, kit);
        return;
    }

    combat.swing_timer -= DT;
    if combat.swing_timer > 0.0 {
        world.insert(player_id, combat);
        world.insert(player_id, kit);
        return;
    }
    combat.swing_timer = PLAYER_SWING_SEC;
    let dmg = combat.attack_damage.max(4.0);
    if matches!(kit.resource_type, Some(ResourceType::Rage)) {
        gain_resource(&mut kit, 5.0);
    }
    world.insert(player_id, combat);
    world.insert(player_id, kit);
    deal_damage(world, player_id, tid, dmg, None, events);
}

pub fn update_mob_combat(
    mob_id: EntityId,
    player_id: EntityId,
    world: &mut World,
    events: &mut Vec<SimEvent>,
) {
    if !is_living_mob(world, mob_id) {
        return;
    }
    let focus = prefer_mob_target(world, mob_id, 40.0).unwrap_or(player_id);
    if let Some(c) = world.get_mut::<Combat>(mob_id) {
        c.target = Some(focus);
    }
    if !world.get::<Health>(focus).is_some_and(|h| h.alive) {
        if let Some(c) = world.get_mut::<Combat>(mob_id) {
            c.target = None;
        }
        return;
    }
    let d = dist2d_ids(world, mob_id, focus);
    if d > MELEE_RANGE {
        return;
    }
    let yaw = face_toward_ids(world, mob_id, focus);
    if let Some(t) = world.get_mut::<Transform>(mob_id) {
        t.yaw = yaw;
    }
    let dmg = {
        let Some(combat) = world.get_mut::<Combat>(mob_id) else {
            return;
        };
        combat.swing_timer -= DT;
        if combat.swing_timer > 0.0 {
            return;
        }
        combat.swing_timer = MOB_SWING_SEC;
        combat.attack_damage.max(3.0)
    };
    deal_damage(world, mob_id, focus, dmg, None, events);
}

/// Entity-facing aura apply for modules not yet cut to World (consumables).
pub fn apply_aura_entity(target: &mut Entity, aura: AuraInstance, events: &mut Vec<SimEvent>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::AuraInstance;
    use woc_content::PlayerClass;
    use woc_protocol::AbilitySlot;

    fn sync_world(entities: &[Entity]) -> crate::ecs::World {
        let mut world = crate::ecs::World::new();
        for e in entities {
            crate::ecs::spawn::sync_entity_to_world(&mut world, e);
        }
        world
    }

    fn apply_world(world: &crate::ecs::World, entities: &mut [Entity]) {
        crate::ecs::spawn::apply_world_to_entities(world, entities);
    }

    fn run_player_combat(
        entities: &mut [Entity],
        slot: Option<AbilitySlot>,
        events: &mut Vec<SimEvent>,
    ) {
        let mut world = sync_world(entities);
        update_player_combat(1, &mut world, slot, events);
        apply_world(&world, entities);
    }

    fn run_tick_auras(entities: &mut [Entity], events: &mut Vec<SimEvent>) {
        let mut world = sync_world(entities);
        tick_auras(&mut world, events);
        apply_world(&world, entities);
    }

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

        run_player_combat(&mut entities, Some(AbilitySlot::Primary), &mut events);
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
        run_player_combat(&mut entities, Some(AbilitySlot::Primary), &mut events);
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
        run_player_combat(&mut entities, Some(AbilitySlot::Primary), &mut events);
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

        run_tick_auras(&mut entities, &mut events);
        assert_eq!(entities[0].auras.len(), 1, "aura still active mid-duration");
        assert!(entities[0].auras[0].remaining > 0.0);

        run_tick_auras(&mut entities, &mut events);
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

        run_tick_auras(&mut entities, &mut events);
        assert!(entities[0].hp < start_hp, "DoT should tick once");
        let after_tick = entities[0].hp;

        run_tick_auras(&mut entities, &mut events);
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

        run_player_combat(&mut entities, Some(AbilitySlot::Primary), &mut events);
        assert!(entities[0].cast.is_some(), "fireball should begin casting");
        assert_eq!(entities[1].hp, start_hp, "no damage until cast completes");
        assert!(entities[0].gcd > 0.0);

        // Advance cast to completion.
        let duration = entities[0].cast.as_ref().unwrap().duration;
        let ticks = (duration / DT).ceil() as u32 + 1;
        for _ in 0..ticks {
            run_player_combat(&mut entities, None, &mut events);
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

        run_player_combat(&mut entities, Some(AbilitySlot::Slot2), &mut events);
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

        run_player_combat(&mut entities, Some(AbilitySlot::Slot2), &mut events);
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

        run_player_combat(&mut entities, Some(AbilitySlot::Primary), &mut events);
        let after_primary = entities[1].hp;
        assert!(after_primary < 500.0);

        // Clear GCD only; primary CD remains. Slot3 should still fire.
        entities[0].gcd = 0.0;
        entities[0].resource = 100.0;
        entities[0].auto_attack = false;
        entities[0].swing_timer = 99.0;
        events.clear();
        run_player_combat(&mut entities, Some(AbilitySlot::Slot3), &mut events);
        assert!(
            entities[1].hp < after_primary,
            "slot3 execute should ignore primary ability CD"
        );
    }
}
