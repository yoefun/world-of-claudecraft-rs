//! Combat: auto-attack, primary ability, GCD, casts, auras, damage, death, XP, loot.

use crate::ecs::components::{AuraInstance, CastState};
use crate::ecs::components::{
    Auras, Bags, ClassKit, Combat, Equipment, EquipmentWear, Health, Identity, LootPile, LootTable,
    Owner, Progress, Threat, Transform,
};
use crate::ecs::World;
use crate::rng::Rng;
use crate::stats::recalc_player_stats;
use crate::types::{
    CRIT_CHANCE, CRIT_MULT, MELEE_RANGE, MISS_CHANCE, MOB_SWING_SEC, PLAYER_SWING_SEC,
    RAGE_FROM_TAKEN, RANGED_FALLBACK, STEALTH_MOVE_MULT,
};
use woc_content::{
    ability, aura_for_ability, class_ability_for_slot, item, mob, AbilityDef, AbilityEffect,
    ResourceType,
};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, EquipSlot, SimEvent, DT};

/// Global cooldown after starting an ability (seconds).
pub const GCD_SEC: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitResult {
    Miss,
    Hit,
    Crit,
}

pub fn roll_hit(rng: &mut Rng) -> HitResult {
    roll_hit_with_crit(rng, CRIT_CHANCE)
}

pub fn roll_hit_with_crit(rng: &mut Rng, crit_chance: f32) -> HitResult {
    let r = rng.next_f32();
    if r < MISS_CHANCE {
        HitResult::Miss
    } else if r < MISS_CHANCE + crit_chance.max(0.0) {
        HitResult::Crit
    } else {
        HitResult::Hit
    }
}

fn roll_player_hit(world: &World, rng: &mut Rng, src: EntityId) -> HitResult {
    let crit = (CRIT_CHANCE + crate::talents::talent_bonus(world, src, "crit_pct"))
        .clamp(0.0, 1.0 - MISS_CHANCE);
    roll_hit_with_crit(rng, crit)
}

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

/// True while a stun aura remains on `id`.
pub fn is_stunned(world: &World, id: EntityId) -> bool {
    world
        .get::<Auras>(id)
        .is_some_and(|store| store.auras.iter().any(|a| a.stun && a.remaining > 0.0))
}

pub fn is_stealthed(world: &World, id: EntityId) -> bool {
    world.get::<ClassKit>(id).is_some_and(|k| k.stealthed)
}

/// Slowest remaining slow (<1) times fastest haste (>1), then stealth min.
pub fn move_speed_mult(world: &World, id: EntityId) -> f32 {
    let aura_mult = world
        .get::<Auras>(id)
        .map(|store| {
            let mut slow = 1.0_f32;
            let mut haste = 1.0_f32;
            for aura in store.auras.iter().filter(|a| a.remaining > 0.0) {
                if aura.move_mult < 1.0 {
                    slow = slow.min(aura.move_mult);
                } else if aura.move_mult > 1.0 {
                    haste = haste.max(aura.move_mult);
                }
            }
            (slow * haste).max(0.0)
        })
        .unwrap_or(1.0);
    if is_stealthed(world, id) {
        aura_mult.min(STEALTH_MOVE_MULT)
    } else {
        aura_mult
    }
}

pub fn remaining_absorb(world: &World, id: EntityId) -> f32 {
    world
        .get::<Auras>(id)
        .map(|store| store.auras.iter().map(|a| a.absorb.max(0.0)).sum())
        .unwrap_or(0.0)
}

pub fn toggle_stealth(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    if class != Some(woc_content::PlayerClass::Rogue) {
        events.push(SimEvent::Toast {
            message: "You cannot stealth.".into(),
        });
        return;
    }
    let Some(kit) = world.get_mut::<ClassKit>(player_id) else {
        return;
    };
    kit.stealthed = !kit.stealthed;
    let message = if kit.stealthed {
        "You enter stealth."
    } else {
        "You leave stealth."
    };
    events.push(SimEvent::Toast {
        message: message.into(),
    });
}

const THORNS_REBOUND: &str = "lightning_shield";

fn aura_armor_flat(world: &World, id: EntityId) -> f32 {
    world
        .get::<Auras>(id)
        .map(|store| {
            store
                .auras
                .iter()
                .filter(|a| a.remaining > 0.0)
                .map(|a| a.armor_flat.max(0.0))
                .sum()
        })
        .unwrap_or(0.0)
}

fn melee_thorns(world: &World, id: EntityId) -> f32 {
    world
        .get::<Auras>(id)
        .map(|store| {
            store
                .auras
                .iter()
                .filter(|a| a.remaining > 0.0)
                .map(|a| a.thorns.max(0.0))
                .sum()
        })
        .unwrap_or(0.0)
}

fn remove_named_auras(world: &mut World, id: EntityId, names: &[&str]) {
    if let Some(store) = world.get_mut::<Auras>(id) {
        store.auras.retain(|a| !names.iter().any(|n| a.id == *n));
    }
}

fn instance_from_def(source: EntityId, def: &woc_content::AuraDef) -> AuraInstance {
    AuraInstance {
        id: def.id.into(),
        remaining: def.duration,
        stacks: 1,
        tick_timer: def.tick_interval.max(0.01),
        tick_interval: def.tick_interval,
        tick_damage: def.tick_damage,
        tick_heal: def.tick_heal,
        source,
        stun: def.stun,
        move_mult: def.move_mult,
        absorb: def.absorb,
        breaks_on_damage: def.breaks_on_damage,
        damage_mult: def.damage_mult,
        thorns: def.thorns,
        armor_flat: def.armor_flat,
    }
}

pub fn apply_named_aura(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    aura_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(def) = woc_content::aura(aura_id) else {
        return;
    };
    apply_aura(world, target, instance_from_def(source, def), events);
}

/// Paladin devotion, warrior battle/defensive, restored shaman/druid forms.
pub fn apply_spawn_identity(world: &mut World, player_id: EntityId) {
    let mut events = Vec::new();
    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    let stance = world
        .get::<ClassKit>(player_id)
        .and_then(|k| k.stance_id.clone());
    match class {
        Some(woc_content::PlayerClass::Paladin) => {
            apply_named_aura(world, player_id, player_id, "devotion_aura", &mut events);
        }
        Some(woc_content::PlayerClass::Warrior) => {
            if stance.as_deref() == Some("defensive") {
                apply_named_aura(world, player_id, player_id, "defensive_stance", &mut events);
            } else {
                if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
                    kit.stance_id = Some("battle".into());
                }
                apply_named_aura(world, player_id, player_id, "battle_shout", &mut events);
            }
        }
        Some(woc_content::PlayerClass::Shaman) if stance.as_deref() == Some("ghost_wolf") => {
            apply_named_aura(world, player_id, player_id, "ghost_wolf", &mut events);
        }
        Some(woc_content::PlayerClass::Druid) if stance.as_deref() == Some("travel_form") => {
            apply_named_aura(world, player_id, player_id, "travel_form", &mut events);
        }
        _ => {}
    }
}

pub fn cycle_stance(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    if class != Some(woc_content::PlayerClass::Warrior) {
        events.push(SimEvent::Toast {
            message: "You cannot change stance.".into(),
        });
        return;
    }
    let current = world
        .get::<ClassKit>(player_id)
        .and_then(|k| k.stance_id.clone());
    let next = if current.as_deref() == Some("defensive") {
        "battle"
    } else {
        "defensive"
    };
    if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
        kit.stance_id = Some(next.into());
    }
    remove_named_auras(world, player_id, &["battle_shout", "defensive_stance"]);
    if next == "battle" {
        apply_named_aura(world, player_id, player_id, "battle_shout", events);
        events.push(SimEvent::Toast {
            message: "Battle Stance.".into(),
        });
    } else {
        apply_named_aura(world, player_id, player_id, "defensive_stance", events);
        events.push(SimEvent::Toast {
            message: "Defensive Stance.".into(),
        });
    }
}

pub fn toggle_form(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    let form_id = match class {
        Some(woc_content::PlayerClass::Shaman) => "ghost_wolf",
        Some(woc_content::PlayerClass::Druid) => "travel_form",
        _ => {
            events.push(SimEvent::Toast {
                message: "You cannot change form.".into(),
            });
            return;
        }
    };
    let current = world
        .get::<ClassKit>(player_id)
        .and_then(|k| k.stance_id.clone());
    if current.as_deref() == Some(form_id) {
        if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
            kit.stance_id = None;
        }
        remove_named_auras(world, player_id, &[form_id]);
        events.push(SimEvent::Toast {
            message: "You return to humanoid form.".into(),
        });
        return;
    }
    if let Some(kit) = world.get_mut::<ClassKit>(player_id) {
        kit.stance_id = Some(form_id.into());
    }
    remove_named_auras(world, player_id, &["ghost_wolf", "travel_form"]);
    apply_named_aura(world, player_id, player_id, form_id, events);
    let message = if form_id == "ghost_wolf" {
        "You shift into Ghost Wolf."
    } else {
        "You shift into Travel Form."
    };
    events.push(SimEvent::Toast {
        message: message.into(),
    });
}

#[derive(Clone, Copy)]
enum AbilityAim {
    Harm(EntityId),
    Help(EntityId),
}

impl AbilityAim {
    fn target(self) -> EntityId {
        match self {
            AbilityAim::Harm(id) | AbilityAim::Help(id) => id,
        }
    }

    fn starts_auto_attack(self) -> bool {
        matches!(self, AbilityAim::Harm(_))
    }
}

fn is_hot_ability(def: &AbilityDef) -> bool {
    aura_for_ability(def.id).is_some_and(|a| a.is_hot())
}

fn is_self_buff_ability(def: &AbilityDef) -> bool {
    aura_for_ability(def.id).is_some_and(|a| a.is_self_buff())
}

fn target_hp_pct(world: &World, id: EntityId) -> Option<f32> {
    let h = world.get::<Health>(id)?;
    (h.hp_max > 1e-3).then_some(h.hp / h.hp_max)
}

fn aim_ability(
    world: &World,
    src: EntityId,
    def: &AbilityDef,
    requested: Option<EntityId>,
    hostile: Option<EntityId>,
) -> Option<AbilityAim> {
    match def.effect {
        AbilityEffect::Heal { .. } | AbilityEffect::Absorb { .. } => {
            Some(AbilityAim::Help(heal_target(world, src, requested)))
        }
        AbilityEffect::Blink { .. } | AbilityEffect::Convert { .. } => Some(AbilityAim::Help(src)),
        AbilityEffect::HealOrHarm { .. } => {
            if let Some(tid) = hostile {
                Some(AbilityAim::Harm(tid))
            } else {
                Some(AbilityAim::Help(heal_target(world, src, requested)))
            }
        }
        AbilityEffect::Execute { hp_pct, .. } => {
            let tid = hostile?;
            let pct = target_hp_pct(world, tid)?;
            (pct <= hp_pct).then_some(AbilityAim::Harm(tid))
        }
        AbilityEffect::ApplyAura if is_hot_ability(def) || is_self_buff_ability(def) => {
            Some(AbilityAim::Help(heal_target(world, src, requested)))
        }
        AbilityEffect::AoeDamage { .. } if def.flags.self_aoe => Some(AbilityAim::Help(src)),
        _ => hostile.map(AbilityAim::Harm),
    }
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
    melee_swing: bool,
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
    if melee_swing && source != target && world.get::<Bags>(target).is_some() {
        wear_player_armor(world, target, events);
    }
    let talent_mult = world
        .get::<Progress>(source)
        .map(|p| crate::talents::damage_multiplier_from_ranks(&p.talents))
        .unwrap_or(1.0);
    let aura_mult = world
        .get::<Auras>(source)
        .map(|store| {
            store
                .auras
                .iter()
                .filter(|a| a.remaining > 0.0)
                .map(|a| a.damage_mult)
                .fold(1.0_f32, |acc, m| acc * m)
        })
        .unwrap_or(1.0);
    let armor = world.get::<Combat>(target).map(|c| c.armor).unwrap_or(0.0)
        + aura_armor_flat(world, target);
    let mitigated = (amount * talent_mult * aura_mult - armor * 0.05).max(1.0);

    let mut remaining = mitigated;
    let mut popped = Vec::new();
    if let Some(store) = world.get_mut::<Auras>(target) {
        for aura in store.auras.iter_mut() {
            if remaining <= 0.0 {
                break;
            }
            if aura.absorb > 0.0 {
                let soak = remaining.min(aura.absorb);
                aura.absorb -= soak;
                remaining -= soak;
                if aura.absorb <= 1e-4 {
                    popped.push(aura.id.clone());
                }
            }
        }
        store.auras.retain(|a| !popped.iter().any(|id| id == &a.id));
        store.auras.retain(|a| !a.breaks_on_damage);
    }
    let rebound = if ability_name.is_none() {
        melee_thorns(world, target)
    } else {
        0.0
    };

    if remaining > 0.0 {
        let Some(health) = world.get_mut::<Health>(target) else {
            return;
        };
        health.hp = (health.hp - remaining).max(0.0);
        let died = health.hp <= 0.0;
        if died {
            health.alive = false;
        }
    }
    let died = world.get::<Health>(target).is_some_and(|h| !h.alive);
    if remaining > 0.0 {
        if let Some(kit) = world.get_mut::<ClassKit>(target) {
            if kit.resource_type == Some(ResourceType::Rage) {
                gain_resource(kit, remaining * RAGE_FROM_TAKEN);
            }
        }
    }
    if let Some(kit) = world.get_mut::<ClassKit>(target) {
        kit.stealthed = false;
    }
    add_threat(world, target, source, mitigated);
    events.push(SimEvent::Damage {
        source,
        target,
        amount: remaining.max(0.0),
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
    if rebound > 0.0 && source != target && world.get::<Health>(source).is_some_and(|h| h.alive) {
        deal_damage(
            world,
            target,
            source,
            rebound,
            Some(THORNS_REBOUND),
            false,
            events,
        );
    }
}

fn equipment_item_id(equipment: &Equipment, slot: EquipSlot) -> Option<&str> {
    match slot {
        EquipSlot::MainHand => equipment.main_hand.as_deref(),
        EquipSlot::OffHand => equipment.off_hand.as_deref(),
        EquipSlot::Head => equipment.head.as_deref(),
        EquipSlot::Chest => equipment.chest.as_deref(),
        EquipSlot::Legs => equipment.legs.as_deref(),
        EquipSlot::Feet => equipment.feet.as_deref(),
    }
}

fn equipment_wear_slot_mut(wear: &mut EquipmentWear, slot: EquipSlot) -> &mut Option<u32> {
    match slot {
        EquipSlot::MainHand => &mut wear.main_hand,
        EquipSlot::OffHand => &mut wear.off_hand,
        EquipSlot::Head => &mut wear.head,
        EquipSlot::Chest => &mut wear.chest,
        EquipSlot::Legs => &mut wear.legs,
        EquipSlot::Feet => &mut wear.feet,
    }
}

fn decrement_wear(
    world: &mut World,
    player_id: EntityId,
    slot: EquipSlot,
    events: &mut Vec<SimEvent>,
) {
    let mut broken_item_name = None;
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        let Some(item_id) = equipment_item_id(&bags.equipment, slot).map(str::to_string) else {
            return;
        };
        let Some(def) = item(&item_id) else {
            return;
        };
        if def.max_durability == 0 {
            return;
        }

        let wear_slot = equipment_wear_slot_mut(&mut bags.equipment_wear, slot);
        let before = wear_slot.unwrap_or(def.max_durability);
        let after = before.saturating_sub(1);
        *wear_slot = Some(after);
        if before > 0 && after == 0 {
            broken_item_name = Some(def.name.to_string());
        }
    }

    if let Some(item_name) = broken_item_name {
        events.push(SimEvent::Toast {
            message: format!("Your {item_name} is broken."),
        });
        recalc_player_stats(world, player_id);
    }
}

pub fn wear_player_weapon(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    decrement_wear(world, player_id, EquipSlot::MainHand, events);
}

pub fn wear_player_armor(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    for slot in [
        EquipSlot::Head,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::Feet,
        EquipSlot::OffHand,
    ] {
        decrement_wear(world, player_id, slot, events);
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
            existing.stun = aura.stun;
            existing.move_mult = aura.move_mult;
            existing.absorb = existing.absorb.max(aura.absorb);
            existing.breaks_on_damage = aura.breaks_on_damage;
            existing.damage_mult = aura.damage_mult;
            existing.thorns = aura.thorns;
            existing.armor_flat = aura.armor_flat;
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

fn apply_ability_aura(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    ability_id: &str,
    events: &mut Vec<SimEvent>,
) {
    if !world.get::<Health>(target).is_some_and(|h| h.alive) {
        return;
    }
    let Some(def) = aura_for_ability(ability_id) else {
        return;
    };
    apply_aura(world, target, instance_from_def(source, def), events);
    if def.stun {
        if let Some(c) = world.get_mut::<Combat>(target) {
            c.cast = None;
            c.auto_attack = false;
        }
    }
}

fn scale_hit(amount: f32, hit: HitResult) -> Option<f32> {
    match hit {
        HitResult::Miss => None,
        HitResult::Hit => Some(amount),
        HitResult::Crit => Some(amount * CRIT_MULT),
    }
}

fn toast_miss(events: &mut Vec<SimEvent>, name: &str) {
    events.push(SimEvent::Toast {
        message: format!("{name} misses."),
    });
}

fn toast_crit(events: &mut Vec<SimEvent>, name: &str) {
    events.push(SimEvent::Toast {
        message: format!("{name} crits!"),
    });
}

fn apply_direct_damage(
    world: &mut World,
    rng: &mut Rng,
    src: EntityId,
    tid: EntityId,
    def: &AbilityDef,
    base: f32,
    events: &mut Vec<SimEvent>,
) {
    let hit = roll_player_hit(world, rng, src);
    match scale_hit(base, hit) {
        None => toast_miss(events, def.name),
        Some(amount) => {
            if hit == HitResult::Crit {
                toast_crit(events, def.name);
            }
            deal_damage(world, src, tid, amount, Some(def.name), false, events);
            apply_ability_aura(world, src, tid, def.id, events);
            add_combo_on_hit(world, src, def);
        }
    }
}

fn aoe_targets(
    world: &World,
    src: EntityId,
    primary: EntityId,
    radius: f32,
    max_targets: u32,
) -> Vec<EntityId> {
    let origin = world
        .get::<Transform>(primary)
        .copied()
        .or_else(|| world.get::<Transform>(src).copied());
    let Some(origin) = origin else {
        return Vec::new();
    };
    let mut ids: Vec<(f32, EntityId)> = world
        .ids::<LootTable>()
        .into_iter()
        .filter(|&id| world.get::<Health>(id).is_some_and(|h| h.alive))
        .filter_map(|id| {
            let t = world.get::<Transform>(id)?;
            let d = dist2d_pose(origin.x, origin.z, t.x, t.z);
            (d <= radius).then_some((d, id))
        })
        .collect();
    ids.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(pos) = ids.iter().position(|(_, id)| *id == primary) {
        ids.swap(0, pos);
    } else if is_living_mob(world, primary) {
        ids.insert(0, (0.0, primary));
    }
    ids.into_iter()
        .map(|(_, id)| id)
        .take(max_targets as usize)
        .collect()
}

fn heal_target(world: &World, src: EntityId, requested: Option<EntityId>) -> EntityId {
    if let Some(tid) = requested {
        if is_living_friendly(world, src, tid) {
            return tid;
        }
    }
    src
}

fn is_living_hostile(world: &World, src: EntityId, tid: EntityId) -> bool {
    if tid == src {
        return false;
    }
    if !world.get::<Health>(tid).is_some_and(|h| h.alive) {
        return false;
    }
    if world.get::<LootTable>(tid).is_some() {
        return true;
    }
    if world.get::<ClassKit>(tid).is_some() {
        let src_flag = world
            .get::<Progress>(src)
            .map(|p| p.pvp_flagged)
            .unwrap_or(false);
        let tid_flag = world
            .get::<Progress>(tid)
            .map(|p| p.pvp_flagged)
            .unwrap_or(false);
        return src_flag && tid_flag;
    }
    false
}

fn is_living_friendly(world: &World, src: EntityId, tid: EntityId) -> bool {
    if !world.get::<Health>(tid).is_some_and(|h| h.alive) {
        return false;
    }
    if tid == src {
        return true;
    }
    if world.get::<Owner>(tid).is_some_and(|o| o.owner_id == src) {
        return true;
    }
    world.get::<ClassKit>(tid).is_some() && !is_living_hostile(world, src, tid)
}

fn add_combo_on_hit(world: &mut World, src: EntityId, def: &AbilityDef) {
    if def.flags.combo_add == 0 {
        return;
    }
    if let Some(kit) = world.get_mut::<ClassKit>(src) {
        kit.combo_points = kit.combo_points.saturating_add(def.flags.combo_add).min(5);
    }
}

fn consume_combo_and_rage_scale(world: &mut World, src: EntityId, def: &AbilityDef) -> f32 {
    let mut scale = 1.0;
    if def.flags.combo_spend {
        let combo = world
            .get::<ClassKit>(src)
            .map(|k| k.combo_points)
            .unwrap_or(0);
        scale *= 1.0 + def.flags.combo_per_point * f32::from(combo);
        if let Some(kit) = world.get_mut::<ClassKit>(src) {
            kit.combo_points = 0;
        }
    }
    if def.flags.rage_dump {
        if let Some(kit) = world.get_mut::<ClassKit>(src) {
            let spent = kit.resource;
            kit.resource = 0.0;
            if kit.resource_max > 1e-3 {
                scale *= 1.0 + spent / kit.resource_max;
            }
        }
    }
    scale
}

fn apply_charge(
    world: &mut World,
    rng: &mut Rng,
    src: EntityId,
    tid: EntityId,
    def: &AbilityDef,
    base: f32,
    events: &mut Vec<SimEvent>,
) {
    let gap = match def.effect {
        AbilityEffect::Charge { gap } => gap,
        _ => 25.0,
    };
    let d = dist2d_ids(world, src, tid);
    if d > gap + 1e-3 {
        events.push(SimEvent::Toast {
            message: "Out of range.".into(),
        });
        return;
    }
    if d > MELEE_RANGE {
        let Some(target) = world.get::<Transform>(tid).copied() else {
            return;
        };
        let speed = (gap / DT).max(crate::types::RUN_SPEED);
        let _ = crate::entity_motion::step_toward(world, src, target.x, target.z, speed);
        if dist2d_ids(world, src, tid) > MELEE_RANGE {
            events.push(SimEvent::Toast {
                message: "Charge blocked.".into(),
            });
            return;
        }
    }
    apply_direct_damage(world, rng, src, tid, def, base, events);
}

fn apply_blink(world: &mut World, src: EntityId, distance: f32) {
    let Some(t) = world.get::<Transform>(src).copied() else {
        return;
    };
    let wish_x = t.x + distance * t.yaw.sin();
    let wish_z = t.z + distance * t.yaw.cos();
    let (nx, nz) = crate::world::clamp_to_world(wish_x, wish_z);
    let ny = crate::world::ground_height(nx, nz, crate::world::WORLD_SEED);
    if let Some(t) = world.get_mut::<Transform>(src) {
        t.x = nx;
        t.z = nz;
        t.y = ny;
    }
}

fn apply_convert(
    world: &mut World,
    src: EntityId,
    hp_cost: f32,
    resource_gain: f32,
    events: &mut Vec<SimEvent>,
) {
    if let Some(health) = world.get_mut::<Health>(src) {
        if !health.alive {
            return;
        }
        health.hp = (health.hp - hp_cost).max(1.0);
    }
    if let Some(kit) = world.get_mut::<ClassKit>(src) {
        gain_resource(kit, resource_gain);
    }
    events.push(SimEvent::Toast {
        message: "Life Tap.".into(),
    });
}

fn apply_absorb_shield(
    world: &mut World,
    src: EntityId,
    requested: Option<EntityId>,
    def: &AbilityDef,
    amount: f32,
    events: &mut Vec<SimEvent>,
) {
    let tid = heal_target(world, src, requested);
    apply_ability_aura(world, src, tid, def.id, events);
    if let Some(store) = world.get_mut::<Auras>(tid) {
        if let Some(aura) = store
            .auras
            .iter_mut()
            .rev()
            .find(|a| def.aura.is_some_and(|id| a.id == id))
        {
            aura.absorb = amount;
        }
    }
}

pub fn apply_ability_effect(
    world: &mut World,
    rng: &mut Rng,
    src: EntityId,
    def: &AbilityDef,
    events: &mut Vec<SimEvent>,
) {
    let attack = world
        .get::<Combat>(src)
        .map(|c| c.attack_damage)
        .unwrap_or(0.0);
    let requested = world.get::<Combat>(src).and_then(|c| c.target);
    let weapon = def.damage + attack * 0.35;
    let rage = world
        .get::<ClassKit>(src)
        .and_then(|k| k.resource_type)
        .is_some_and(|rt| matches!(rt, ResourceType::Rage));
    if rage {
        if let Some(kit) = world.get_mut::<ClassKit>(src) {
            gain_resource(kit, 5.0);
        }
    }
    let dmg_scale = consume_combo_and_rage_scale(world, src, def);

    match def.effect {
        AbilityEffect::WeaponDamage { coefficient } => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            apply_direct_damage(
                world,
                rng,
                src,
                tid,
                def,
                weapon * coefficient * dmg_scale,
                events,
            );
        }
        AbilityEffect::SpellDamage { .. } => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            apply_direct_damage(world, rng, src, tid, def, weapon * dmg_scale, events);
        }
        AbilityEffect::AoeDamage {
            radius,
            max_targets,
        } => {
            let primary = if def.flags.self_aoe {
                src
            } else {
                match requested.filter(|&t| is_living_hostile(world, src, t)) {
                    Some(tid) => tid,
                    None => return,
                }
            };
            let hit = roll_player_hit(world, rng, src);
            let Some(amount) = scale_hit(weapon * dmg_scale, hit) else {
                toast_miss(events, def.name);
                return;
            };
            if hit == HitResult::Crit {
                toast_crit(events, def.name);
            }
            let extra =
                crate::talents::talent_bonus(world, src, "cleave_targets_plus").max(0.0) as u32;
            let cap = max_targets.saturating_add(extra);
            for tid in aoe_targets(world, src, primary, radius, cap) {
                if tid == src {
                    continue;
                }
                deal_damage(world, src, tid, amount, Some(def.name), false, events);
                apply_ability_aura(world, src, tid, def.id, events);
            }
        }
        AbilityEffect::Heal { coefficient } => {
            let tid = heal_target(world, src, requested);
            apply_direct_heal(world, rng, src, tid, def, coefficient, events);
        }
        AbilityEffect::HealOrHarm { coefficient } => {
            if let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) {
                apply_direct_damage(
                    world,
                    rng,
                    src,
                    tid,
                    def,
                    weapon * coefficient * dmg_scale,
                    events,
                );
            } else {
                let tid = heal_target(world, src, requested);
                apply_direct_heal(world, rng, src, tid, def, coefficient, events);
            }
        }
        AbilityEffect::Execute {
            hp_pct,
            coefficient,
        } => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            let Some(pct) = target_hp_pct(world, tid) else {
                return;
            };
            if pct > hp_pct {
                return;
            }
            apply_direct_damage(
                world,
                rng,
                src,
                tid,
                def,
                weapon * coefficient * dmg_scale,
                events,
            );
        }
        AbilityEffect::ApplyAura => {
            let tid = if is_hot_ability(def) || is_self_buff_ability(def) {
                heal_target(world, src, requested)
            } else {
                match requested.filter(|&t| is_living_hostile(world, src, t)) {
                    Some(tid) => tid,
                    None => return,
                }
            };
            if is_hot_ability(def) || is_self_buff_ability(def) {
                apply_ability_aura(world, src, tid, def.id, events);
            } else {
                let hit = roll_player_hit(world, rng, src);
                if let Some(amount) = scale_hit(weapon.max(1.0) * dmg_scale, hit) {
                    if hit == HitResult::Crit {
                        toast_crit(events, def.name);
                    }
                    if def.damage > 0.0 {
                        deal_damage(world, src, tid, amount, Some(def.name), false, events);
                    }
                    apply_ability_aura(world, src, tid, def.id, events);
                    add_combo_on_hit(world, src, def);
                } else {
                    toast_miss(events, def.name);
                }
            }
        }
        AbilityEffect::Interrupt => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            if let Some(c) = world.get_mut::<Combat>(tid) {
                c.cast = None;
                c.cast_lockout = def.flags.interrupt_lockout.max(1.5);
            }
            events.push(SimEvent::Toast {
                message: format!("{name} interrupts!", name = def.name),
            });
            apply_direct_damage(world, rng, src, tid, def, weapon * dmg_scale, events);
        }
        AbilityEffect::Taunt { threat } => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            add_threat(world, tid, src, threat);
            if let Some(c) = world.get_mut::<Combat>(tid) {
                c.target = Some(src);
            }
            events.push(SimEvent::Toast {
                message: format!("{name} taunts.", name = def.name),
            });
        }
        AbilityEffect::Absorb { amount } => {
            apply_absorb_shield(world, src, requested, def, amount, events);
        }
        AbilityEffect::Charge { .. } => {
            let Some(tid) = requested.filter(|&t| is_living_hostile(world, src, t)) else {
                return;
            };
            apply_charge(world, rng, src, tid, def, weapon * dmg_scale, events);
        }
        AbilityEffect::Blink { distance } => {
            apply_blink(world, src, distance);
            events.push(SimEvent::Toast {
                message: "Blink.".into(),
            });
        }
        AbilityEffect::Convert {
            hp_cost,
            resource_gain,
        } => {
            apply_convert(world, src, hp_cost, resource_gain, events);
        }
    }
}

fn apply_direct_heal(
    world: &mut World,
    rng: &mut Rng,
    src: EntityId,
    tid: EntityId,
    def: &AbilityDef,
    coefficient: f32,
    events: &mut Vec<SimEvent>,
) {
    let hit = roll_player_hit(world, rng, src);
    let heal_mult = 1.0 + crate::talents::talent_bonus(world, src, "heal_pct");
    let amount = match hit {
        HitResult::Miss | HitResult::Hit => def.damage * coefficient * heal_mult,
        HitResult::Crit => {
            toast_crit(events, def.name);
            def.damage * coefficient * CRIT_MULT * heal_mult
        }
    };
    apply_heal(world, tid, amount, def.name, events);
    apply_ability_aura(world, src, tid, def.id, events);
}

fn apply_heal(
    world: &mut World,
    target: EntityId,
    amount: f32,
    ability_name: &str,
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
            message: format!("{ability_name} heals for {:.0}.", healed),
        });
    }
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
        deal_damage(world, source, target, amount, Some(&aura_id), false, events);
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

pub fn try_pickup_loot(
    player_id: EntityId,
    world: &mut World,
    events: &mut Vec<SimEvent>,
    pending: &crate::social::LootRules,
) {
    if world.get::<ClassKit>(player_id).is_none() {
        return;
    }
    let loot_ids: Vec<EntityId> = world
        .ids::<LootPile>()
        .into_iter()
        .filter(|&id| {
            if pending.is_pending(id) {
                return false;
            }
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
        let _ = grant_loot_pile(world, player_id, lid, events);
    }
}

/// Claim a specific loot pile (or loot near a corpse) via Interact.
pub fn claim_loot_target(
    player_id: EntityId,
    target_id: EntityId,
    world: &mut World,
    events: &mut Vec<SimEvent>,
    pending: &crate::social::LootRules,
) -> bool {
    if !world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false)
    {
        return false;
    }

    if world.get::<LootPile>(target_id).is_some() {
        if crate::ecs::components::dist2d(world, player_id, target_id)
            .map(|d| d > crate::types::INTERACT_RANGE)
            .unwrap_or(true)
        {
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
        if world
            .get::<Identity>(target_id)
            .and_then(|i| i.template_id.as_deref())
            .and_then(woc_content::gather_node)
            .is_some()
        {
            return false;
        }
        return grant_loot_pile(world, player_id, target_id, events);
    }

    // Dead mob corpse: vacuum nearby loot piles.
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Mob)
        || world.get::<Health>(target_id).is_some_and(|h| h.alive)
    {
        return false;
    }
    if crate::ecs::components::dist2d(world, player_id, target_id)
        .map(|d| d > crate::types::INTERACT_RANGE)
        .unwrap_or(true)
    {
        events.push(SimEvent::Toast {
            message: "Too far to loot.".into(),
        });
        return false;
    }
    let Some(corpse) = world.get::<Transform>(target_id).cloned() else {
        return false;
    };
    let loot_ids: Vec<EntityId> = world
        .ids::<LootPile>()
        .into_iter()
        .filter(|&id| {
            if pending.is_pending(id) {
                return false;
            }
            if world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                .and_then(woc_content::gather_node)
                .is_some()
            {
                return false;
            }
            world
                .get::<Transform>(id)
                .map(|t| {
                    let dx = t.x - corpse.x;
                    let dz = t.z - corpse.z;
                    (dx * dx + dz * dz).sqrt() < crate::types::LOOT_RANGE
                })
                .unwrap_or(false)
        })
        .collect();
    if loot_ids.is_empty() {
        let pending_near = world.ids::<LootPile>().into_iter().any(|id| {
            pending.is_pending(id)
                && world
                    .get::<Transform>(id)
                    .map(|t| {
                        let dx = t.x - corpse.x;
                        let dz = t.z - corpse.z;
                        (dx * dx + dz * dz).sqrt() < crate::types::LOOT_RANGE
                    })
                    .unwrap_or(false)
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
        let _ = grant_loot_pile(world, player_id, lid, events);
    }
    events.len() > before
}

fn grant_loot_pile(
    world: &mut World,
    player_id: EntityId,
    lid: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(pile) = world.get::<LootPile>(lid).cloned() else {
        return false;
    };
    if let Some(ref it) = pile.item {
        if crate::inventory::grant_item(world, player_id, it, 1, events).is_err() {
            events.push(SimEvent::Toast {
                message: "Inventory full.".into(),
            });
            return false;
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
    true
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

fn is_living_mob(world: &World, id: EntityId) -> bool {
    world.get::<LootTable>(id).is_some() && world.get::<Health>(id).is_some_and(|h| h.alive)
}

pub fn update_player_combat(
    player_id: EntityId,
    world: &mut World,
    ability_slot: Option<AbilitySlot>,
    rng: &mut Rng,
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
    if combat.cast_lockout > 0.0 {
        combat.cast_lockout = (combat.cast_lockout - DT).max(0.0);
    }

    if is_stunned(world, player_id) {
        combat.cast = None;
        combat.auto_attack = false;
        world.insert(player_id, combat);
        world.insert(player_id, kit);
        return;
    }

    if combat
        .target
        .is_some_and(|tid| !world.get::<Health>(tid).is_some_and(|h| h.alive))
    {
        combat.target = None;
        combat.auto_attack = false;
        combat.cast = None;
    }

    let tid = combat.target;
    let hostile = tid.filter(|&t| is_living_hostile(world, player_id, t));
    let d_hostile = hostile.map(|t| dist2d_ids(world, player_id, t));

    if let Some(t) = hostile {
        let yaw = face_toward_ids(world, player_id, t);
        if let Some(tr) = world.get_mut::<Transform>(player_id) {
            tr.yaw = yaw;
        }
    }

    let range = ability_range(&kit).max(MELEE_RANGE);
    let in_melee = d_hostile.map(|d| d <= MELEE_RANGE).unwrap_or(false);

    if combat.cast.is_some() {
        let cast_range = combat
            .cast
            .as_ref()
            .and_then(|c| ability(&c.ability_id))
            .map(|a| a.range)
            .unwrap_or(range);
        let cast_target = combat.cast.as_ref().map(|c| c.target);
        let in_cast_range = cast_target
            .map(|ct| {
                dist2d_ids(world, player_id, ct) <= cast_range.max(RANGED_FALLBACK.min(cast_range))
            })
            .unwrap_or(false);
        if !in_cast_range {
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
                    apply_ability_effect(world, rng, player_id, def, events);
                }
                combat = world.get::<Combat>(player_id).cloned().unwrap_or(combat);
                kit = world.get::<ClassKit>(player_id).cloned().unwrap_or(kit);
            } else {
                combat.cast = Some(cast);
            }
        }
    } else if let Some(slot) = ability_slot {
        if combat.gcd <= 0.0 && combat.cast_lockout <= 0.0 {
            if let Some(def) = resolve_slot_ability(&kit, slot) {
                let abil_id = def.id;
                let abil_range = def.range.max(RANGED_FALLBACK.min(def.range));
                if def.flags.requires_stealth && !kit.stealthed {
                    events.push(SimEvent::Toast {
                        message: "Must be stealthed.".into(),
                    });
                } else if let Some(aim) = aim_ability(world, player_id, def, tid, hostile) {
                    let aim_tid = aim.target();
                    let in_slot_range =
                        aim_tid == player_id || dist2d_ids(world, player_id, aim_tid) <= abil_range;
                    if in_slot_range
                        && !ability_on_cd(&kit, abil_id)
                        && spend_resource(&mut kit, def.cost)
                    {
                        if def.flags.breaks_stealth {
                            kit.stealthed = false;
                        }
                        start_ability_cd(&mut kit, &mut combat, abil_id, def.cooldown);
                        combat.gcd = GCD_SEC;
                        if aim.starts_auto_attack() {
                            combat.auto_attack = true;
                        }
                        let cast_tid = aim_tid;
                        if def.cast_time > 0.0 {
                            combat.cast = Some(CastState {
                                ability_id: abil_id.to_string(),
                                elapsed: 0.0,
                                duration: def.cast_time,
                                target: cast_tid,
                            });
                        } else {
                            world.insert(player_id, combat.clone());
                            world.insert(player_id, kit.clone());
                            apply_ability_effect(world, rng, player_id, def, events);
                            return;
                        }
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
    if let Some(tid) = hostile {
        let hit = roll_player_hit(world, rng, player_id);
        match scale_hit(dmg, hit) {
            None => toast_miss(events, "Auto-attack"),
            Some(amount) => {
                if hit == HitResult::Crit {
                    toast_crit(events, "Auto-attack");
                }
                wear_player_weapon(world, player_id, events);
                deal_damage(world, player_id, tid, amount, None, true, events);
            }
        }
    }
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
    if is_stunned(world, mob_id) {
        return;
    }
    if let Some(c) = world.get_mut::<Combat>(mob_id) {
        if c.cast_lockout > 0.0 {
            c.cast_lockout = (c.cast_lockout - DT).max(0.0);
        }
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
    deal_damage(world, mob_id, focus, dmg, None, true, events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Auras, Bags, ClassKit, Combat, Health, Transform};
    use woc_content::PlayerClass;
    use woc_protocol::AbilitySlot;

    fn hit_rng() -> Rng {
        Rng::new(1)
    }

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
    fn player_swing_wears_main_hand() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Swinger", PlayerClass::Warrior, 0.0, 0.0);
        let before = world
            .get::<Bags>(1)
            .unwrap()
            .equipment_wear
            .main_hand
            .unwrap();
        let mut events = Vec::new();

        crate::combat::wear_player_weapon(&mut world, 1, &mut events);

        let after = world
            .get::<Bags>(1)
            .unwrap()
            .equipment_wear
            .main_hand
            .unwrap();
        assert_eq!(after, before - 1);
    }

    #[test]
    fn mob_hit_wears_armor_not_weapon() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tank", PlayerClass::Warrior, 0.0, 0.0);
        let weapon_before = world
            .get::<Bags>(1)
            .unwrap()
            .equipment_wear
            .main_hand
            .unwrap();
        let chest_before = world.get::<Bags>(1).unwrap().equipment_wear.chest.unwrap();
        let mut events = Vec::new();

        crate::combat::wear_player_armor(&mut world, 1, &mut events);

        let bags = world.get::<Bags>(1).unwrap();
        assert_eq!(bags.equipment_wear.main_hand.unwrap(), weapon_before);
        assert_eq!(bags.equipment_wear.chest.unwrap(), chest_before - 1);
    }

    #[test]
    fn gcd_blocks_second_ability_cast() {
        let mut world = player_and_mob();
        let mob_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();

        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Primary),
            &mut hit_rng(),
            &mut events,
        );
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Primary),
            &mut hit_rng(),
            &mut events,
        );
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Primary),
            &mut hit_rng(),
            &mut events,
        );
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
                    stun: false,
                    move_mult: 1.0,
                    absorb: 0.0,
                    breaks_on_damage: false,
                    damage_mult: 1.0,
                    thorns: 0.0,
                    armor_flat: 0.0,
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
                    stun: false,
                    move_mult: 1.0,
                    absorb: 0.0,
                    breaks_on_damage: false,
                    damage_mult: 1.0,
                    thorns: 0.0,
                    armor_flat: 0.0,
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Primary),
            &mut hit_rng(),
            &mut events,
        );
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
            update_player_combat(1, &mut world, None, &mut hit_rng(), &mut events);
        }
        assert!(world.get::<Combat>(1).unwrap().cast.is_none());
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
        assert!(world
            .get::<Auras>(2)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "ignite"));
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot2),
            &mut hit_rng(),
            &mut events,
        );
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot2),
            &mut hit_rng(),
            &mut events,
        );
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
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Primary),
            &mut hit_rng(),
            &mut events,
        );
        let after_primary = world.get::<Health>(2).unwrap().hp;
        assert!(after_primary < 500.0);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = h.hp_max * 0.15;
        }
        let wounded = world.get::<Health>(2).unwrap().hp;
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        events.clear();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot3),
            &mut hit_rng(),
            &mut events,
        );
        assert!(world.get::<Health>(2).unwrap().hp < wounded);
    }

    #[test]
    fn seeded_hit_table_yields_miss_and_crit() {
        let mut rng = Rng::new(1);
        let mut miss = 0;
        let mut crit = 0;
        let mut hit = 0;
        for _ in 0..200 {
            match roll_hit(&mut rng) {
                HitResult::Miss => miss += 1,
                HitResult::Crit => crit += 1,
                HitResult::Hit => hit += 1,
            }
        }
        assert!(miss >= 1, "expected a miss in 200 rolls, got {miss}");
        assert!(crit >= 1, "expected a crit in 200 rolls, got {crit}");
        assert!(hit >= 1, "expected a hit in 200 rolls, got {hit}");
    }

    #[test]
    fn cleave_damages_two_wolves_in_radius() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 1.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 2.0, 0.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        let hp2 = world.get::<Health>(2).unwrap().hp;
        let hp3 = world.get::<Health>(3).unwrap().hp;
        let mut events = Vec::new();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot2),
            &mut hit_rng(),
            &mut events,
        );
        assert!(world.get::<Health>(2).unwrap().hp < hp2, "primary wolf");
        assert!(world.get::<Health>(3).unwrap().hp < hp3, "cleave splash");
    }

    #[test]
    fn warrior_cleave_talent_hits_one_extra_target() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 1.0, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 1.2, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 4, "young_wolf", 1.4, 0.0).unwrap();
        crate::ecs::spawn::create_mob_from_template(&mut world, 5, "young_wolf", 1.6, 0.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(p) = world.get_mut::<crate::ecs::components::Progress>(1) {
            p.talent_points = 1;
        }
        let mut events = Vec::new();
        assert!(crate::talents::learn(
            &mut world,
            1,
            "warrior_improved_cleave",
            &mut events
        ));
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        let start: Vec<f32> = (2..=5)
            .map(|id| world.get::<Health>(id).unwrap().hp)
            .collect();
        events.clear();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot2),
            &mut hit_rng(),
            &mut events,
        );
        let hit_count = (2..=5)
            .filter(|&id| world.get::<Health>(id).unwrap().hp < start[(id - 2) as usize])
            .count();
        assert_eq!(
            hit_count, 4,
            "cleave_targets_plus should raise max_targets from 3 to 4"
        );
    }

    #[test]
    fn priest_heal_restores_party_member() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Priest", PlayerClass::Priest, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Warrior", PlayerClass::Warrior, 1.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 10.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.gcd = 0.0;
            c.auto_attack = false;
        }
        let mut events = Vec::new();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot4),
            &mut hit_rng(),
            &mut events,
        );
        assert!(
            world.get::<Health>(2).unwrap().hp > 10.0,
            "flash heal should land on the targeted player"
        );
    }

    #[test]
    fn interrupt_cancels_mob_cast() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Shaman", PlayerClass::Shaman, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 1.0, 0.0).unwrap();
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = 3;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.cast = Some(CastState {
                ability_id: "bite".into(),
                elapsed: 0.2,
                duration: 2.0,
                target: 1,
            });
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        let mut events = Vec::new();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot2),
            &mut hit_rng(),
            &mut events,
        );
        assert!(
            world.get::<Combat>(2).unwrap().cast.is_none(),
            "earth shock interrupts"
        );
    }

    #[test]
    fn taunt_sets_mob_target_and_threat() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tank", PlayerClass::Warrior, 0.0, 0.0);
        crate::ecs::spawn::create_player(&mut world, 2, "Dps", PlayerClass::Mage, 1.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 3, "young_wolf", 1.5, 0.0).unwrap();
        add_threat(&mut world, 3, 2, 40.0);
        if let Some(c) = world.get_mut::<Combat>(3) {
            c.target = Some(2);
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(3);
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        let mut events = Vec::new();
        update_player_combat(
            1,
            &mut world,
            Some(AbilitySlot::Slot4),
            &mut hit_rng(),
            &mut events,
        );
        assert_eq!(world.get::<Combat>(3).unwrap().target, Some(1));
        let threat = &world
            .get::<crate::ecs::components::Threat>(3)
            .unwrap()
            .threat;
        assert!(threat.get(&1).copied().unwrap_or(0.0) >= 80.0);
    }

    fn class_and_mob(class: PlayerClass, level: u32) -> World {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Tester", class, 0.0, 0.0);
        crate::ecs::spawn::create_mob_from_template(&mut world, 2, "young_wolf", 1.0, 0.0)
            .expect("wolf");
        if let Some(h) = world.get_mut::<Health>(1) {
            h.level = level;
        }
        crate::ecs::spawn::refresh_known_abilities(&mut world, 1);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.ability_cd = 0.0;
            c.gcd = 0.0;
            c.target = Some(2);
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        world
    }

    fn fire_slot(world: &mut World, slot: AbilitySlot) -> Vec<SimEvent> {
        let mut events = Vec::new();
        update_player_combat(1, world, Some(slot), &mut hit_rng(), &mut events);
        events
    }

    fn finish_cast(world: &mut World) {
        let duration = world
            .get::<Combat>(1)
            .and_then(|c| c.cast.as_ref().map(|c| c.duration))
            .unwrap_or(0.0);
        let ticks = (duration / DT).ceil() as u32 + 2;
        let mut events = Vec::new();
        for _ in 0..ticks {
            update_player_combat(1, world, None, &mut hit_rng(), &mut events);
        }
    }

    #[test]
    fn execute_requires_wounded_target() {
        let mut world = class_and_mob(PlayerClass::Warrior, 6);
        let start = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert_eq!(
            world.get::<Health>(2).unwrap().hp,
            start,
            "execute must not land at full HP"
        );
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 80.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(world.get::<Health>(2).unwrap().hp < 80.0);
    }

    #[test]
    fn heroic_strike_does_not_apply_rend() {
        let mut world = class_and_mob(PlayerClass::Warrior, 1);
        fire_slot(&mut world, AbilitySlot::Primary);
        let has_rend = world
            .get::<Auras>(2)
            .map(|a| a.auras.iter().any(|aura| aura.id == "rend"))
            .unwrap_or(false);
        assert!(
            !has_rend,
            "rend is its own ability, not a heroic strike rider"
        );
    }

    #[test]
    fn serpent_sting_applies_named_dot() {
        let mut world = class_and_mob(PlayerClass::Hunter, 3);
        fire_slot(&mut world, AbilitySlot::Slot2);
        assert!(
            world
                .get::<Auras>(2)
                .unwrap()
                .auras
                .iter()
                .any(|a| a.id == "serpent_sting"),
            "serpent sting should apply its own DoT"
        );
    }

    #[test]
    fn holy_shock_heals_ally_and_damages_foe() {
        let mut world = class_and_mob(PlayerClass::Paladin, 6);
        crate::ecs::spawn::create_player(&mut world, 3, "Ally", PlayerClass::Warrior, 1.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(3) {
            h.hp = 10.0;
        }
        let wolf = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(world.get::<Health>(2).unwrap().hp < wolf, "holy shock harm");

        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(3);
            c.gcd = 0.0;
            c.auto_attack = false;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(world.get::<Health>(3).unwrap().hp > 10.0, "holy shock heal");
    }

    #[test]
    fn cheap_shot_stuns_mob() {
        let mut world = class_and_mob(PlayerClass::Rogue, 6);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.stealthed = true;
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(is_stunned(&world, 2), "cheap shot stuns");
        let player_hp = world.get::<Health>(1).unwrap().hp;
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.target = Some(1);
            c.swing_timer = 0.0;
        }
        let mut events = Vec::new();
        update_mob_combat(2, 1, &mut world, &mut events);
        assert_eq!(
            world.get::<Health>(1).unwrap().hp,
            player_hp,
            "stunned mob must not swing"
        );
    }

    #[test]
    fn frostbolt_applies_chill_slow() {
        let mut world = class_and_mob(PlayerClass::Mage, 3);
        fire_slot(&mut world, AbilitySlot::Slot2);
        finish_cast(&mut world);
        let chill = world
            .get::<Auras>(2)
            .unwrap()
            .auras
            .iter()
            .find(|a| a.id == "chill")
            .expect("chill");
        assert!((chill.move_mult - 0.5).abs() < 1e-3);
        assert!((move_speed_mult(&world, 2) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn paladin_holy_light_heals() {
        let mut world = class_and_mob(PlayerClass::Paladin, 1);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 20.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        fire_slot(&mut world, AbilitySlot::Slot4);
        assert!(world.get::<Health>(1).unwrap().hp > 20.0);
    }

    #[test]
    fn shaman_and_druid_can_heal() {
        for (class, slot) in [
            (PlayerClass::Shaman, AbilitySlot::Slot4),
            (PlayerClass::Druid, AbilitySlot::Slot4),
        ] {
            let mut world = class_and_mob(class, 1);
            if let Some(h) = world.get_mut::<Health>(1) {
                h.hp = 15.0;
            }
            if let Some(c) = world.get_mut::<Combat>(1) {
                c.target = None;
            }
            fire_slot(&mut world, slot);
            assert!(
                world.get::<Health>(1).unwrap().hp > 15.0,
                "{class:?} heal slot should restore HP"
            );
        }
    }

    #[test]
    fn absorb_soaks_damage_before_hp() {
        let mut world = player_and_mob();
        world.insert(
            2,
            Auras {
                auras: vec![AuraInstance {
                    id: "power_word_shield".into(),
                    remaining: 15.0,
                    stacks: 1,
                    tick_timer: 99.0,
                    tick_interval: 0.0,
                    tick_damage: 0.0,
                    tick_heal: 0.0,
                    source: 1,
                    stun: false,
                    move_mult: 1.0,
                    absorb: 30.0,
                    breaks_on_damage: false,
                    damage_mult: 1.0,
                    thorns: 0.0,
                    armor_flat: 0.0,
                }],
            },
        );
        let start_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();
        deal_damage(&mut world, 1, 2, 20.0, Some("Smite"), false, &mut events);
        assert_eq!(world.get::<Health>(2).unwrap().hp, start_hp);
        let absorb = world.get::<Auras>(2).unwrap().auras[0].absorb;
        assert!(absorb < 30.0 && absorb > 0.0, "partial soak, left {absorb}");
    }

    #[test]
    fn interrupt_sets_cast_lockout() {
        let mut world = class_and_mob(PlayerClass::Shaman, 3);
        if let Some(c) = world.get_mut::<Combat>(2) {
            c.cast = Some(CastState {
                ability_id: "bite".into(),
                elapsed: 0.2,
                duration: 2.0,
                target: 1,
            });
        }
        fire_slot(&mut world, AbilitySlot::Slot2);
        assert!(world.get::<Combat>(2).unwrap().cast.is_none());
        assert!(world.get::<Combat>(2).unwrap().cast_lockout >= 1.5 - 1e-3);
    }

    #[test]
    fn self_aoe_fires_without_hostile_target() {
        let mut world = class_and_mob(PlayerClass::Mage, 3);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        let start = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot4);
        assert!(
            world.get::<Health>(2).unwrap().hp < start,
            "frost nova should hit nearby wolves without a target"
        );
        assert!(world
            .get::<Auras>(2)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "chill"));
    }

    #[test]
    fn rage_increases_when_warrior_is_hit() {
        let mut world = player_and_mob();
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 0.0;
        }
        let mut events = Vec::new();
        deal_damage(&mut world, 2, 1, 40.0, None, true, &mut events);
        assert!(
            world.get::<ClassKit>(1).unwrap().resource > 0.0,
            "warrior should gain rage from taken damage"
        );
    }

    #[test]
    fn execute_dumps_remaining_rage() {
        let mut world = class_and_mob(PlayerClass::Warrior, 6);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp_max = 2000.0;
            h.hp = 300.0;
            h.alive = true;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 25.0;
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        let low_rage_hp = world.get::<Health>(2).unwrap().hp;
        assert!(low_rage_hp < 300.0);
        assert!(world.get::<Health>(2).unwrap().alive);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 300.0;
            h.alive = true;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 100.0;
            kit.ability_cds.clear();
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        let dumped = world.get::<Health>(2).unwrap().hp;
        assert!(
            dumped < low_rage_hp,
            "dumping leftover rage should increase execute damage ({dumped} vs {low_rage_hp})"
        );
        assert_eq!(world.get::<ClassKit>(1).unwrap().resource, 0.0);
    }

    #[test]
    fn combo_builder_and_spend() {
        let mut world = class_and_mob(PlayerClass::Rogue, 3);
        fire_slot(&mut world, AbilitySlot::Primary);
        assert_eq!(world.get::<ClassKit>(1).unwrap().combo_points, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        fire_slot(&mut world, AbilitySlot::Primary);
        assert_eq!(world.get::<ClassKit>(1).unwrap().combo_points, 2);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
        }
        let before = world.get::<Health>(2).unwrap().hp;
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
            kit.combo_points = 5;
        }
        fire_slot(&mut world, AbilitySlot::Slot2);
        assert_eq!(world.get::<ClassKit>(1).unwrap().combo_points, 0);
        assert!(world.get::<Health>(2).unwrap().hp < before);
    }

    #[test]
    fn charge_closes_gap_then_hits() {
        let mut world = player_and_mob();
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = 0.0;
            t.z = 0.0;
        }
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 8.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        let start_x = world.get::<Transform>(1).unwrap().x;
        let start_hp = world.get::<Health>(2).unwrap().hp;
        let def = ability("charge").expect("charge");
        let mut events = Vec::new();
        apply_ability_effect(&mut world, &mut hit_rng(), 1, def, &mut events);
        assert!(
            world.get::<Transform>(1).unwrap().x > start_x + 0.5,
            "charge should close toward the wolf"
        );
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
        assert!(dist2d_ids(&world, 1, 2) <= MELEE_RANGE + 0.15);
    }

    #[test]
    fn blink_displaces_along_facing() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Mage", PlayerClass::Mage, 0.0, 0.0);
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.yaw = 0.0;
        }
        let start = world.get::<Transform>(1).copied().unwrap();
        let def = ability("blink").expect("blink");
        let mut events = Vec::new();
        apply_ability_effect(&mut world, &mut hit_rng(), 1, def, &mut events);
        let after = world.get::<Transform>(1).unwrap();
        let dz = after.z - start.z;
        assert!(
            dz.abs() > 5.0,
            "blink along yaw 0 should move on +z, got dz={dz}"
        );
    }

    #[test]
    fn life_tap_converts_hp_to_mana() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Lock", PlayerClass::Warlock, 0.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 80.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 20.0;
        }
        let def = ability("life_tap").expect("life_tap");
        let mut events = Vec::new();
        apply_ability_effect(&mut world, &mut hit_rng(), 1, def, &mut events);
        assert!(world.get::<Health>(1).unwrap().hp < 80.0);
        assert!(world.get::<Health>(1).unwrap().hp >= 1.0);
        assert!(world.get::<ClassKit>(1).unwrap().resource > 20.0);
    }

    #[test]
    fn cheap_shot_requires_stealth() {
        let mut world = class_and_mob(PlayerClass::Rogue, 6);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.stealthed = false;
        }
        let start = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert_eq!(world.get::<Health>(2).unwrap().hp, start);
        assert!(!is_stunned(&world, 2));
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.stealthed = true;
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(is_stunned(&world, 2));
        assert!(!world.get::<ClassKit>(1).unwrap().stealthed);
    }

    #[test]
    fn rogue_cheap_shot_fails_without_stealth() {
        let mut world = class_and_mob(PlayerClass::Rogue, 6);
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.stealthed = false;
        }
        fire_slot(&mut world, AbilitySlot::Slot3);
        assert!(!is_stunned(&world, 2));
    }

    #[test]
    fn rogue_eviscerate_scales_with_combo() {
        let mut world = class_and_mob(PlayerClass::Rogue, 3);
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 800.0;
            h.hp_max = 800.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.combo_points = 0;
        }
        fire_slot(&mut world, AbilitySlot::Slot2);
        let zero_combo_hp = world.get::<Health>(2).unwrap().hp;
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 800.0;
            h.alive = true;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
            kit.combo_points = 5;
        }
        fire_slot(&mut world, AbilitySlot::Slot2);
        let five_combo_hp = world.get::<Health>(2).unwrap().hp;
        assert!(
            five_combo_hp < zero_combo_hp,
            "eviscerate should scale with combo ({five_combo_hp} vs {zero_combo_hp})"
        );
        assert_eq!(world.get::<ClassKit>(1).unwrap().combo_points, 0);
    }

    #[test]
    fn priest_shield_on_slot5() {
        let mut world = class_and_mob(PlayerClass::Priest, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        fire_slot(&mut world, AbilitySlot::Slot5);
        let absorb = remaining_absorb(&world, 1);
        assert!(
            absorb > 0.0,
            "priest slot 5 should apply Power Word: Shield absorb, got {absorb}"
        );
    }

    #[test]
    fn warrior_charge_from_eight_yards() {
        let mut world = class_and_mob(PlayerClass::Warrior, 1);
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.x = 0.0;
            t.z = 0.0;
        }
        if let Some(t) = world.get_mut::<Transform>(2) {
            t.x = 8.0;
            t.z = 0.0;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        let start_x = world.get::<Transform>(1).unwrap().x;
        let start_hp = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot5);
        assert!(
            world.get::<Transform>(1).unwrap().x > start_x + 0.5,
            "charge on slot 5 should close from eight yards"
        );
        assert!(world.get::<Health>(2).unwrap().hp < start_hp);
    }

    #[test]
    fn mage_frost_nova_without_target() {
        let mut world = class_and_mob(PlayerClass::Mage, 3);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        let start = world.get::<Health>(2).unwrap().hp;
        fire_slot(&mut world, AbilitySlot::Slot4);
        assert!(world.get::<Health>(2).unwrap().hp < start);
    }

    #[test]
    fn mage_blink_on_slot5() {
        let mut world = class_and_mob(PlayerClass::Mage, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        if let Some(t) = world.get_mut::<Transform>(1) {
            t.yaw = 0.0;
        }
        let start_z = world.get::<Transform>(1).unwrap().z;
        fire_slot(&mut world, AbilitySlot::Slot5);
        assert!(
            (world.get::<Transform>(1).unwrap().z - start_z).abs() > 5.0,
            "mage slot 5 should blink along facing"
        );
    }

    #[test]
    fn hunter_spends_mana_not_energy() {
        let mut world = class_and_mob(PlayerClass::Hunter, 1);
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().resource_type,
            Some(woc_content::ResourceType::Mana)
        );
        let before = world.get::<ClassKit>(1).unwrap().resource;
        fire_slot(&mut world, AbilitySlot::Primary);
        let after = world.get::<ClassKit>(1).unwrap().resource;
        assert!(
            after < before,
            "arcane shot should spend hunter mana ({after} vs {before})"
        );
    }

    #[test]
    fn hunter_aspect_buffs_outgoing_damage() {
        let mut world = class_and_mob(PlayerClass::Hunter, 1);
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = None;
        }
        fire_slot(&mut world, AbilitySlot::Slot5);
        assert!(
            world
                .get::<Auras>(1)
                .unwrap()
                .auras
                .iter()
                .any(|a| a.id == "aspect_of_the_hawk"),
            "hunter slot 5 should apply Aspect of the Hawk"
        );
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.hp_max = 500.0;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.target = Some(2);
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        let with_aspect = {
            let start = world.get::<Health>(2).unwrap().hp;
            fire_slot(&mut world, AbilitySlot::Primary);
            start - world.get::<Health>(2).unwrap().hp
        };
        if let Some(store) = world.get_mut::<Auras>(1) {
            store.auras.clear();
        }
        if let Some(h) = world.get_mut::<Health>(2) {
            h.hp = 500.0;
            h.alive = true;
        }
        if let Some(c) = world.get_mut::<Combat>(1) {
            c.gcd = 0.0;
            c.auto_attack = false;
            c.swing_timer = 99.0;
        }
        if let Some(kit) = world.get_mut::<ClassKit>(1) {
            kit.resource = 200.0;
            kit.ability_cds.clear();
        }
        let without_aspect = {
            let start = world.get::<Health>(2).unwrap().hp;
            fire_slot(&mut world, AbilitySlot::Primary);
            start - world.get::<Health>(2).unwrap().hp
        };
        assert!(
            with_aspect > without_aspect + 0.5,
            "aspect should raise outgoing damage ({with_aspect} vs {without_aspect})"
        );
    }

    fn test_aura(id: &str) -> AuraInstance {
        AuraInstance {
            id: id.into(),
            remaining: 30.0,
            stacks: 1,
            tick_timer: 99.0,
            tick_interval: 0.0,
            tick_damage: 0.0,
            tick_heal: 0.0,
            source: 1,
            stun: false,
            move_mult: 1.0,
            absorb: 0.0,
            breaks_on_damage: false,
            damage_mult: 1.0,
            thorns: 0.0,
            armor_flat: 0.0,
        }
    }

    #[test]
    fn lightning_shield_thorns_hits_attacker() {
        let mut world = class_and_mob(PlayerClass::Shaman, 1);
        let mut shield = test_aura("lightning_shield");
        shield.thorns = 8.0;
        apply_aura(&mut world, 1, shield, &mut Vec::new());
        let attacker_hp = world.get::<Health>(2).unwrap().hp;
        let mut events = Vec::new();
        deal_damage(&mut world, 2, 1, 12.0, None, true, &mut events);
        assert!(
            world.get::<Health>(2).unwrap().hp < attacker_hp,
            "melee attacker should take lightning shield thorns"
        );
        let after_melee = world.get::<Health>(2).unwrap().hp;
        deal_damage(&mut world, 2, 1, 12.0, Some("Bite"), false, &mut events);
        assert_eq!(
            world.get::<Health>(2).unwrap().hp,
            after_melee,
            "named spell hits must not proc thorns"
        );
    }

    #[test]
    fn fear_breaks_when_damaged() {
        let mut world = class_and_mob(PlayerClass::Warlock, 1);
        let mut fear = test_aura("fear");
        fear.stun = true;
        fear.move_mult = 0.0;
        fear.breaks_on_damage = true;
        apply_aura(&mut world, 2, fear, &mut Vec::new());
        assert!(is_stunned(&world, 2));
        let mut events = Vec::new();
        deal_damage(&mut world, 1, 2, 10.0, Some("Shadow Bolt"), false, &mut events);
        assert!(
            !is_stunned(&world, 2),
            "fear should break when the target is damaged"
        );
    }

    #[test]
    fn travel_form_speeds_then_breaks_on_hit() {
        let mut world = class_and_mob(PlayerClass::Druid, 1);
        let mut form = test_aura("travel_form");
        form.move_mult = 1.4;
        form.breaks_on_damage = true;
        apply_aura(&mut world, 1, form, &mut Vec::new());
        assert!(
            (move_speed_mult(&world, 1) - 1.4).abs() < 1e-3,
            "travel form should raise move speed, got {}",
            move_speed_mult(&world, 1)
        );
        let mut events = Vec::new();
        deal_damage(&mut world, 2, 1, 10.0, None, true, &mut events);
        assert!(
            world
                .get::<Auras>(1)
                .unwrap()
                .auras
                .iter()
                .all(|a| a.id != "travel_form"),
            "travel form should break on damage"
        );
        assert!((move_speed_mult(&world, 1) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn defensive_stance_reduces_damage() {
        let mut world = class_and_mob(PlayerClass::Warrior, 1);
        let baseline = {
            let start = world.get::<Health>(1).unwrap().hp;
            deal_damage(&mut world, 2, 1, 40.0, Some("Bite"), false, &mut Vec::new());
            start - world.get::<Health>(1).unwrap().hp
        };
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = h.hp_max;
            h.alive = true;
        }
        let mut stance = test_aura("defensive_stance");
        stance.damage_mult = 0.9;
        stance.armor_flat = 20.0;
        apply_aura(&mut world, 1, stance, &mut Vec::new());
        let reduced = {
            let start = world.get::<Health>(1).unwrap().hp;
            deal_damage(&mut world, 2, 1, 40.0, Some("Bite"), false, &mut Vec::new());
            start - world.get::<Health>(1).unwrap().hp
        };
        assert!(
            reduced + 0.5 < baseline,
            "defensive stance armor_flat should cut incoming damage ({reduced} vs {baseline})"
        );
    }

    #[test]
    fn life_tap_and_fear_on_warlock_bar() {
        assert_eq!(
            woc_content::class_ability_for_slot(PlayerClass::Warlock, 4)
                .expect("warlock 4")
                .id,
            "life_tap"
        );
        assert_eq!(
            woc_content::class_ability_for_slot(PlayerClass::Warlock, 5)
                .expect("warlock 5")
                .id,
            "fear"
        );
        let mut world = class_and_mob(PlayerClass::Warlock, 1);
        fire_slot(&mut world, AbilitySlot::Slot5);
        assert!(
            is_stunned(&world, 2),
            "warlock slot 5 Fear should stun the wolf"
        );
    }

    #[test]
    fn cycle_stance_applies_defensive_then_battle() {
        let mut world = class_and_mob(PlayerClass::Warrior, 1);
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("battle")
        );
        cycle_stance(&mut world, 1, &mut Vec::new());
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("defensive")
        );
        assert!(world
            .get::<Auras>(1)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "defensive_stance"));
        cycle_stance(&mut world, 1, &mut Vec::new());
        assert_eq!(
            world.get::<ClassKit>(1).unwrap().stance_id.as_deref(),
            Some("battle")
        );
        assert!(world
            .get::<Auras>(1)
            .unwrap()
            .auras
            .iter()
            .any(|a| a.id == "battle_shout"));
    }

    #[test]
    fn toggle_form_speeds_druid() {
        let mut world = class_and_mob(PlayerClass::Druid, 1);
        toggle_form(&mut world, 1, &mut Vec::new());
        assert!(
            (move_speed_mult(&world, 1) - 1.4).abs() < 1e-3,
            "travel form should raise move speed"
        );
        toggle_form(&mut world, 1, &mut Vec::new());
        assert!((move_speed_mult(&world, 1) - 1.0).abs() < 1e-3);
    }
}
