//! Player stat recalculation from class + gear + talents.

use std::collections::HashMap;

use crate::ecs::components::{Bags, ClassKit, Combat, Health, Progress};
use crate::ecs::World;
use woc_content::{class_def, item, talent};
use woc_protocol::EntityId;

fn add_gear_stats(item_id: &str, ap: &mut f32, armor: &mut f32, weapon_fraction: f32) {
    if let Some(it) = item(item_id) {
        *ap += it.attack_power * weapon_fraction;
        *armor += it.armor;
    }
}

fn talent_sums(talents: &HashMap<String, u32>) -> (f32, f32, f32, f32) {
    let mut max_hp_pct = 0.0;
    let mut armor_pct = 0.0;
    let mut armor_flat = 0.0;
    let mut resource_pct = 0.0;
    for (id, rank) in talents {
        let Some(def) = talent(id) else {
            continue;
        };
        let r = *rank as f32;
        match def.effect {
            "max_hp_pct" => max_hp_pct += def.effect_value * r,
            "armor_pct" => armor_pct += def.effect_value * r,
            "armor_flat" => armor_flat += def.effect_value * r,
            "resource_pct" => resource_pct += def.effect_value * r,
            _ => {}
        }
    }
    (max_hp_pct, armor_pct, armor_flat, resource_pct)
}

pub fn recalc_player_stats(world: &mut World, player_id: EntityId) {
    let Some(class) = world.get::<ClassKit>(player_id).and_then(|k| k.class_id) else {
        return;
    };
    let def = class_def(class);
    let equipment = world
        .get::<Bags>(player_id)
        .map(|b| b.equipment.clone())
        .unwrap_or_default();
    let talents = world
        .get::<Progress>(player_id)
        .map(|p| p.talents.clone())
        .unwrap_or_default();
    let level = world
        .get::<Health>(player_id)
        .map(|h| h.level)
        .unwrap_or(1);

    let mut ap = def.attack_power;
    let mut armor = 0.0_f32;

    if let Some(ref wid) = equipment.main_hand {
        add_gear_stats(wid, &mut ap, &mut armor, 1.0);
    }
    if let Some(ref oid) = equipment.off_hand {
        add_gear_stats(oid, &mut ap, &mut armor, 0.25);
    }
    if let Some(ref hid) = equipment.head {
        add_gear_stats(hid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref cid) = equipment.chest {
        add_gear_stats(cid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref lid) = equipment.legs {
        add_gear_stats(lid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref fid) = equipment.feet {
        add_gear_stats(fid, &mut ap, &mut armor, 0.0);
    }

    let (max_hp_pct, armor_pct, armor_flat, resource_pct) = talent_sums(&talents);
    armor = (armor + armor_flat) * (1.0 + armor_pct);

    let hp_max =
        (crate::types::player_hp(def.base_hp, level) + armor * 0.5) * (1.0 + max_hp_pct);
    let resource_max = def.resource_max * (1.0 + resource_pct);

    let (hp, hp_max_prev) = world
        .get::<Health>(player_id)
        .map(|h| (h.hp, h.hp_max))
        .unwrap_or((hp_max, hp_max));
    let ratio = if hp_max_prev > 0.0 { hp / hp_max_prev } else { 1.0 };
    let new_hp = (hp_max * ratio).clamp(0.0, hp_max);

    if let Some(c) = world.get_mut::<Combat>(player_id) {
        c.attack_damage = ap;
        c.armor = armor;
    }
    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.hp_max = hp_max;
        h.hp = new_hp;
    }
    if let Some(k) = world.get_mut::<ClassKit>(player_id) {
        k.resource_max = resource_max;
        if k.resource > k.resource_max {
            k.resource = k.resource_max;
        }
    }
}
