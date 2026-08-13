//! Combat: auto-attack, primary ability, GCD, casts, auras, damage, death, XP, loot.

use crate::ecs::components::{AuraInstance, CastState};
use crate::ecs::components::{
    Auras, ClassKit, Combat, Health, Identity, LootPile, LootTable, Progress, Threat, Transform,
};
use crate::ecs::World;
use crate::rng::Rng;
use crate::types::{MELEE_RANGE, MOB_SWING_SEC, PLAYER_SWING_SEC, RANGED_FALLBACK};
use woc_content::{ability, class_ability_for_slot, mob, AbilityDef, ResourceType};
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

pub fn dist2d_pose(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

pub fn face_toward_pose(from_x: f32, from_z: f32, to_x: f32, to_z: f32) -> f32 {
    (to_x - from_x).atan2(to_z - from_z)
}

pub(crate) fn dist2d_ids(world: &World, a: EntityId, b: EntityId) -> f32 {
    crate::ecs::components::dist2d(world, a, b).unwrap_or(f32::MAX)
}

pub(crate) fn face_toward_ids(world: &World, from: EntityId, to: EntityId) -> f32 {
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
        if world.get::<ClassKit>(tid).is_some() && world.get::<Health>(tid).is_some_and(|h| h.alive)
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
    if world
        .get::<Identity>(target)
        .is_some_and(|i| i.kind == EntityKind::Npc)
    {
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

pub fn apply_aura(
    world: &mut World,
    target: EntityId,
    aura: AuraInstance,
    events: &mut Vec<SimEvent>,
) {
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
        world.insert(target, Auras { auras: vec![aura] });
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

pub fn collect_pending_mob_kills(events: &[SimEvent], world: &World) -> Vec<KillReward> {
    let mut out = Vec::new();
    for ev in events {
        if let SimEvent::Kill { killer, victim, .. } = ev {
            if world.get::<Identity>(*victim).map(|i| i.kind) != Some(EntityKind::Mob) {
                continue;
            }
            let Some(t) = world.get::<Transform>(*victim) else {
                continue;
            };
            let xp = world
                .get::<LootTable>(*victim)
                .map(|l| l.xp_value)
                .unwrap_or(0);
            let template_id = world
                .get::<Identity>(*victim)
                .and_then(|i| i.template_id.clone());
            out.push(KillReward {
                killer: *killer,
                victim: *victim,
                template_id,
                x: t.x,
                z: t.z,
                xp,
            });
        }
    }
    out
}

pub fn grant_xp(world: &mut World, player_id: EntityId, amount: u32, events: &mut Vec<SimEvent>) {
    crate::quests::grant_xp_world(world, player_id, amount, events);
}

pub fn spawn_mob_loot(
    world: &mut World,
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
    let id = world.next_id();
    crate::ecs::spawn::create_loot(world, id, x, z, copper, item)
}

pub fn try_pickup_loot(player_id: EntityId, world: &mut World, events: &mut Vec<SimEvent>) {
    if world.get::<Identity>(player_id).map(|i| i.kind) != Some(EntityKind::Player) {
        return;
    }
    let loot_ids: Vec<EntityId> = world
        .ids::<LootPile>()
        .into_iter()
        .filter(|&id| {
            let Some(identity) = world.get::<Identity>(id) else {
                return false;
            };
            // Profession gather nodes are harvested via Interact, not auto-loot.
            if identity
                .template_id
                .as_deref()
                .and_then(woc_content::gather_node)
                .is_some()
            {
                return false;
            }
            crate::ecs::components::dist2d(world, player_id, id)
                .map(|d| d < crate::types::LOOT_RANGE)
                .unwrap_or(false)
        })
        .collect();
    for lid in loot_ids {
        let Some(pile) = world.get::<LootPile>(lid).cloned() else {
            continue;
        };
        if let Some(ref it) = pile.item {
            if crate::inventory::grant_item(world, player_id, it, 1, events).is_err() {
                events.push(SimEvent::Toast {
                    message: "Inventory full.".into(),
                });
                continue;
            }
            crate::quests::on_inventory_changed(world, player_id, events);
        }
        world.despawn(lid);
        if let Some(p) = world.get_mut::<Progress>(player_id) {
            p.copper = p.copper.saturating_add(pile.copper);
        }
        events.push(SimEvent::Loot {
            player: player_id,
            copper: pile.copper,
            item: pile.item,
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
    let attack = world
        .get::<Combat>(src)
        .map(|c| c.attack_damage)
        .unwrap_or(0.0);
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
                    resolve_ability_hit(
                        world, player_id, tid, &abil_id, def.name, def.damage, events,
                    );
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
                if in_slot_range
                    && !ability_on_cd(&kit, abil_id)
                    && spend_resource(&mut kit, def.cost)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Auras, ClassKit, Combat, Health};
    use woc_content::PlayerClass;
    use woc_protocol::AbilitySlot;

    fn player_and_mob() -> World {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tester", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 1.0, 0.0)
            .expect("wolf");
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
            kit.primary_ability = Some("heroic_strike".into());
            kit.ability_cds.clear();
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.ability_cd = 0.0;
            c.gcd = 0.0;
            c.target = Some(2);
            c.auto_attack = true;
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        world
    }

    #[test]
    fn gcd_blocks_second_ability_cast() {
        let mut world = player_and_mob();
        let mob_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();

        update_player_combat(1, &mut world, Some(AbilitySlot::Primary), &mut events);
        let after_first = world.get::<Health>(2).unwrap().hp;
        assert!(after_first < mob_hp, "first cast should deal damage");
        assert!(
            world.get::<Combat>(1).unwrap().gcd > 0.0,
            "GCD should start after ability"
        );

        if let Some(c) = world.get_mut::<Combat>(1) {
            c.ability_cd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.ability_cds.clear();
            kit.resource = 100.0;
        }
        let hp_before_second = world.get::<Health>(2).unwrap().hp;
        let auras_before = world.get::<Auras>(2).map(|a| a.auras.len()).unwrap_or(0);
        events.clear();
        update_player_combat(1, &mut world, Some(AbilitySlot::Primary), &mut events);
        assert_eq!(world.get::<Health>(2).unwrap().hp, hp_before_second);
        assert_eq!(
            world.get::<Auras>(2).map(|a| a.auras.len()).unwrap_or(0),
            auras_before
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            SimEvent::Damage {
                ability: Some(_),
                ..
            }
        )));

        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.ability_cd = 0.0;
            c.auto_attack = false;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.ability_cds.clear();
            kit.resource = 100.0;
        }
        update_player_combat(1, &mut world, Some(AbilitySlot::Primary), &mut events);
        assert!(world.get::<Health>(2).unwrap().hp < hp_before_second);
    }

    #[test]
    fn aura_expires_after_remaining_elapses() {
        let mut world = World::new();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        world.insert(
            2,
            Auras {
                auras: vec![AuraInstance {
                    id: "rend".into(),
                    remaining: DT * 1.5,
                    stacks: 1,
                    tick_timer: 999.0,
                    tick_interval: 999.0,
                    tick_damage: 0.0,
                    tick_heal: 0.0,
                    source: 1,
                }],
            },
        );
        let mut events = Vec::new();
        tick_auras(&mut world, &mut events);
        assert_eq!(world.get::<Auras>(2).unwrap().auras.len(), 1);
        tick_auras(&mut world, &mut events);
        assert!(world.get::<Auras>(2).unwrap().auras.is_empty());
    }

    #[test]
    fn aura_dot_ticks_damage_each_interval() {
        let mut world = World::new();
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 0.0, 0.0).unwrap();
        let start_hp = world.get::<Health>(2).unwrap().hp;
        world.insert(
            2,
            Auras {
                auras: vec![AuraInstance {
                    id: "rend".into(),
                    remaining: 10.0,
                    stacks: 1,
                    tick_timer: DT,
                    tick_interval: 3.0 * DT,
                    tick_damage: 7.0,
                    tick_heal: 0.0,
                    source: 1,
                }],
            },
        );
        let mut events = Vec::new();
        tick_auras(&mut world, &mut events);
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
        let after_tick = world.get::<Health>(2).unwrap().hp;
        tick_auras(&mut world, &mut events);
        assert_eq!(world.get::<Health>(2).unwrap().hp, after_tick);
    }

    #[test]
    fn fireball_starts_timed_cast() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Mage", PlayerClass::Mage, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 5.0, 0.0).unwrap();
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
            kit.primary_ability = Some("fireball".into());
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        let start_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();
        update_player_combat(1, &mut world, Some(AbilitySlot::Primary), &mut events);
        assert!(world.get::<Combat>(1).unwrap().cast.is_some());
        assert_eq!(world.get::<Health>(2).unwrap().hp, start_hp);
        assert!(world.get::<Combat>(1).unwrap().gcd > 0.0);
        let duration = world
            .get::<Combat>(1)
            .unwrap()
            .cast
            .as_ref()
            .unwrap()
            .duration;
        let ticks = (duration / DT).ceil() as u32 + 1;
        for _ in 0..ticks {
            update_player_combat(1, &mut world, None, &mut events);
        }
        assert!(world.get::<Combat>(1).unwrap().cast.is_none());
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
        assert!(world
            .get::<Auras>(2)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == IGNITE_ID));
    }

    #[test]
    fn create_player_knows_level_one_kit_abilities() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let known = &world.get::<ClassKit>(1).unwrap().known_abilities;
        assert!(known.iter().any(|a| a == "heroic_strike"));
        assert!(!known.iter().any(|a| a == "cleave"));
        assert!(!known.iter().any(|a| a == "execute"));
    }

    #[test]
    fn level_up_unlocks_gated_kit_abilities() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        grant_xp(&mut world, 1, 10_000, &mut events);
        assert!(world.get::<Health>(1).unwrap().level >= 3);
        assert!(world
            .get::<ClassKit>(1)
            .unwrap()
            .known_abilities
            .iter()
            .any(|a| a == "cleave"));
    }

    #[test]
    fn slot2_ability_deals_damage_when_known() {
        let mut world = player_and_mob();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
            kit.ability_cds.clear();
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.ability_cd = 0.0;
            c.gcd = 0.0;
        }
        let start_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();
        update_player_combat(1, &mut world, Some(AbilitySlot::Slot2), &mut events);
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::Damage { ability: Some(n), .. } if n == "Cleave")));
    }

    #[test]
    fn slot2_blocked_when_ability_unknown() {
        let mut world = player_and_mob();
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        assert_eq!(world.get::<Health>(1).unwrap().level, 1);
        let start_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();
        update_player_combat(1, &mut world, Some(AbilitySlot::Slot2), &mut events);
        assert_eq!(world.get::<Health>(2).unwrap().hp, start_hp);
    }

    #[test]
    fn slot3_and_primary_use_independent_cooldowns() {
        let mut world = player_and_mob();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 6;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        let mut events = Vec::new();
        update_player_combat(1, &mut world, Some(AbilitySlot::Primary), &mut events);
        let after_primary = world.get::<Health>(2).unwrap().hp;
        assert!(after_primary < 500.0);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        events.clear();
        update_player_combat(1, &mut world, Some(AbilitySlot::Slot3), &mut events);
        assert!(world.get::<Health>(2).unwrap().hp < after_primary);
    }
}
