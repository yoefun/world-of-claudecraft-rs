//! Player stat recalculation from class + gear.

use crate::entity::Entity;
use woc_content::{class_def, item};

pub fn recalc_player_stats(player: &mut Entity) {
    let Some(class) = player.class_id else {
        return;
    };
    let def = class_def(class);
    let mut ap = def.attack_power;
    let mut armor = 0.0_f32;

    if let Some(ref wid) = player.equipment.main_hand {
        if let Some(it) = item(wid) {
            ap += it.attack_power;
        }
    }
    if let Some(ref cid) = player.equipment.chest {
        if let Some(it) = item(cid) {
            armor += it.armor;
        }
    }
    if let Some(ref oid) = player.equipment.off_hand {
        if let Some(it) = item(oid) {
            armor += it.armor;
            ap += it.attack_power * 0.25;
        }
    }

    player.attack_damage = ap;
    player.armor = armor;
    let level = player.level;
    let hp_max = crate::types::player_hp(def.base_hp, level) + armor * 0.5;
    let ratio = if player.hp_max > 0.0 {
        player.hp / player.hp_max
    } else {
        1.0
    };
    player.hp_max = hp_max;
    player.hp = (hp_max * ratio).clamp(0.0, hp_max);
    player.resource_max = def.resource_max;
}
