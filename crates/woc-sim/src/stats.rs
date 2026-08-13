//! Player stat recalculation from class + gear + talents.

use std::collections::HashMap;

use crate::ecs::components::{Bags, ClassKit, Combat, Equipment, EquipmentWear, Health, Progress};
use crate::ecs::World;
use woc_content::{class_def, enchant, item, quality_mult, talent};
use woc_protocol::EntityId;

fn add_gear_stats(
    item_id: &str,
    ap: &mut f32,
    armor: &mut f32,
    sta: &mut f32,
    sp: &mut f32,
    weapon_fraction: f32,
) {
    if let Some(it) = item(item_id) {
        let q = quality_mult(it.quality);
        *ap += it.attack_power * q * weapon_fraction;
        *armor += it.armor * q;
        *sta += it.stamina * q;
        *sp += it.spell_power * q;
    }
}

fn slot_broken(wear: Option<u32>) -> bool {
    wear == Some(0)
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
    let (equipment, wear, mh_enchant) = world
        .get::<Bags>(player_id)
        .map(|b| {
            (
                b.equipment.clone(),
                b.equipment_wear.clone(),
                b.equipment_enchants.main_hand.clone(),
            )
        })
        .unwrap_or_else(|| (Equipment::default(), EquipmentWear::default(), None));
    let talents = world
        .get::<Progress>(player_id)
        .map(|p| p.talents.clone())
        .unwrap_or_default();
    let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);

    let mut ap = def.attack_power;
    let mut armor = 0.0_f32;
    let mut sta = 0.0_f32;
    let mut sp = 0.0_f32;

    if !slot_broken(wear.main_hand) {
        if let Some(ref wid) = equipment.main_hand {
            add_gear_stats(wid, &mut ap, &mut armor, &mut sta, &mut sp, 1.0);
        }
    }
    if !slot_broken(wear.off_hand) {
        if let Some(ref oid) = equipment.off_hand {
            add_gear_stats(oid, &mut ap, &mut armor, &mut sta, &mut sp, 0.25);
        }
    }
    if !slot_broken(wear.head) {
        if let Some(ref hid) = equipment.head {
            add_gear_stats(hid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
        }
    }
    if !slot_broken(wear.chest) {
        if let Some(ref cid) = equipment.chest {
            add_gear_stats(cid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
        }
    }
    if !slot_broken(wear.legs) {
        if let Some(ref lid) = equipment.legs {
            add_gear_stats(lid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
        }
    }
    if !slot_broken(wear.feet) {
        if let Some(ref fid) = equipment.feet {
            add_gear_stats(fid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
        }
    }
    // Jewelry: no durability wear columns; always apply when equipped.
    if let Some(ref nid) = equipment.neck {
        add_gear_stats(nid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
    }
    if let Some(ref rid) = equipment.finger {
        add_gear_stats(rid, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
    }
    if let Some(ref r2) = equipment.finger2 {
        add_gear_stats(r2, &mut ap, &mut armor, &mut sta, &mut sp, 0.0);
    }
    if !slot_broken(wear.main_hand) {
        if let Some(ench) = mh_enchant.as_deref().and_then(enchant) {
            ap += ench.attack_power;
            sta += ench.stamina;
            sp += ench.spell_power;
        }
    }

    let (max_hp_pct, armor_pct, armor_flat, resource_pct) = talent_sums(&talents);
    armor = (armor + armor_flat) * (1.0 + armor_pct);

    let hp_max = (crate::types::player_hp(def.base_hp, level) + armor * 0.5 + sta * 2.0)
        * (1.0 + max_hp_pct);
    let resource_max = def.resource_max * (1.0 + resource_pct);

    let (hp, hp_max_prev) = world
        .get::<Health>(player_id)
        .map(|h| (h.hp, h.hp_max))
        .unwrap_or((hp_max, hp_max));
    let ratio = if hp_max_prev > 0.0 {
        hp / hp_max_prev
    } else {
        1.0
    };
    let new_hp = (hp_max * ratio).clamp(0.0, hp_max);

    if let Some(c) = world.get_mut::<Combat>(player_id) {
        c.attack_damage = ap;
        c.armor = armor;
        c.spell_power = sp;
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

#[cfg(test)]
mod tests {
    use super::recalc_player_stats;
    use crate::ecs::components::Bags;
    use crate::ecs::spawn::create_player;
    use crate::ecs::World;
    use woc_content::PlayerClass;

    #[test]
    fn warrior_spawns_full_cloth_extras() {
        let mut world = World::new();
        create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let eq = &world.get::<Bags>(1).unwrap().equipment;
        assert_eq!(eq.main_hand.as_deref(), Some("worn_sword"));
        assert_eq!(eq.chest.as_deref(), Some("recruit_tunic"));
        assert_eq!(eq.head.as_deref(), Some("recruit_cap"));
        assert_eq!(eq.legs.as_deref(), Some("recruit_pants"));
        assert_eq!(eq.feet.as_deref(), Some("recruit_boots"));
        assert!(eq.off_hand.is_none());
        assert!(eq.neck.is_none());
    }

    #[test]
    fn pendant_raises_hp_max() {
        use crate::ecs::components::Health;

        let mut world = World::new();
        create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        let base_hp = world.get::<Health>(1).unwrap().hp_max;
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment.neck = Some("fang_pendant".into());
        }
        recalc_player_stats(&mut world, 1);
        let with_pendant = world.get::<Health>(1).unwrap().hp_max;
        assert!(
            (with_pendant - base_hp - 8.8).abs() < 0.01,
            "expected +8.8 hp_max from sta 4 * uncommon 1.1, got base {base_hp} with {with_pendant}"
        );
    }

    #[test]
    fn hag_focus_raises_spell_power() {
        use crate::ecs::components::Combat;

        let mut world = World::new();
        create_player(&mut world, 1, "P", PlayerClass::Priest, 0.0, 0.0);
        let base_sp = world.get::<Combat>(1).unwrap().spell_power;
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment.neck = Some("hag_focus".into());
        }
        recalc_player_stats(&mut world, 1);
        let with_focus = world.get::<Combat>(1).unwrap().spell_power;
        assert!(
            (with_focus - base_sp - 9.6).abs() < 0.01,
            "expected +9.6 spell_power from rare hag_focus, got base {base_sp} with {with_focus}"
        );
    }

    #[test]
    fn whetstone_enchant_adds_attack_power() {
        use crate::ecs::components::Combat;

        let mut world = World::new();
        create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        recalc_player_stats(&mut world, 1);
        let base = world.get::<Combat>(1).unwrap().attack_damage;
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment_enchants.main_hand = Some("coarse_sharpening".into());
        }
        recalc_player_stats(&mut world, 1);
        let enchanted = world.get::<Combat>(1).unwrap().attack_damage;
        assert!(
            (enchanted - base - 6.0).abs() < 0.01,
            "expected +6 AP from coarse sharpening, got base {base} with {enchanted}"
        );
    }

    #[test]
    fn broken_mh_skips_enchant() {
        use crate::ecs::components::Combat;

        let mut world = World::new();
        create_player(&mut world, 1, "W", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment_enchants.main_hand = Some("coarse_sharpening".into());
            bags.equipment_wear.main_hand = Some(0);
        }
        recalc_player_stats(&mut world, 1);
        let broken = world.get::<Combat>(1).unwrap().attack_damage;
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment_enchants.main_hand = None;
        }
        recalc_player_stats(&mut world, 1);
        let broken_no_enchant = world.get::<Combat>(1).unwrap().attack_damage;
        assert!(
            (broken - broken_no_enchant).abs() < 0.01,
            "broken MH must not receive enchant AP"
        );
    }

    #[test]
    fn broken_weapon_adds_no_attack_power() {
        let mut world = World::new();
        create_player(&mut world, 1, "Worn", PlayerClass::Warrior, 0.0, 0.0);
        recalc_player_stats(&mut world, 1);
        let healthy = world
            .get::<crate::ecs::components::Combat>(1)
            .unwrap()
            .attack_damage;
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.equipment_wear.main_hand = Some(0);
        }
        recalc_player_stats(&mut world, 1);
        let broken = world
            .get::<crate::ecs::components::Combat>(1)
            .unwrap()
            .attack_damage;
        assert!(broken < healthy);
    }
}
