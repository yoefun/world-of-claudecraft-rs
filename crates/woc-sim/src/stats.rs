//! Player stat recalculation from class + gear + talents.

use crate::entity::Entity;
use woc_content::{class_def, item, talent};

fn add_gear_stats(item_id: &str, ap: &mut f32, armor: &mut f32, weapon_fraction: f32) {
    if let Some(it) = item(item_id) {
        *ap += it.attack_power * weapon_fraction;
        *armor += it.armor;
    }
}

fn talent_sums(player: &Entity) -> (f32, f32, f32, f32) {
    let mut max_hp_pct = 0.0;
    let mut armor_pct = 0.0;
    let mut armor_flat = 0.0;
    let mut resource_pct = 0.0;
    for (id, rank) in &player.talents {
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

pub fn recalc_player_stats(player: &mut Entity) {
    let Some(class) = player.class_id else {
        return;
    };
    let def = class_def(class);
    let mut ap = def.attack_power;
    let mut armor = 0.0_f32;

    if let Some(ref wid) = player.equipment.main_hand {
        add_gear_stats(wid, &mut ap, &mut armor, 1.0);
    }
    if let Some(ref oid) = player.equipment.off_hand {
        add_gear_stats(oid, &mut ap, &mut armor, 0.25);
    }
    if let Some(ref hid) = player.equipment.head {
        add_gear_stats(hid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref cid) = player.equipment.chest {
        add_gear_stats(cid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref lid) = player.equipment.legs {
        add_gear_stats(lid, &mut ap, &mut armor, 0.0);
    }
    if let Some(ref fid) = player.equipment.feet {
        add_gear_stats(fid, &mut ap, &mut armor, 0.0);
    }

    let (max_hp_pct, armor_pct, armor_flat, resource_pct) = talent_sums(player);
    armor = (armor + armor_flat) * (1.0 + armor_pct);

    player.attack_damage = ap;
    player.armor = armor;
    let level = player.level;
    let hp_max = (crate::types::player_hp(def.base_hp, level) + armor * 0.5) * (1.0 + max_hp_pct);
    let ratio = if player.hp_max > 0.0 {
        player.hp / player.hp_max
    } else {
        1.0
    };
    player.hp_max = hp_max;
    player.hp = (hp_max * ratio).clamp(0.0, hp_max);
    player.resource_max = def.resource_max * (1.0 + resource_pct);
    if player.resource > player.resource_max {
        player.resource = player.resource_max;
    }
}
