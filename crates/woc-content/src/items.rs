//! Item definitions for the framework slice.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::items_zone2::ZONE2_ITEMS;
use crate::PlayerClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Weapon,
    Armor,
    Consumable,
    Junk,
    Quest,
}

/// Which equipment slot an item occupies when equipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemEquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorClass {
    Cloth,
    Leather,
    Mail,
    Plate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponStyle {
    OneHand,
    TwoHand,
    Ranged,
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipDeny {
    NotGear,
    LevelReq(u32),
    WrongClass,
    WrongArmor,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ItemKind,
    pub stack_size: u32,
    pub vendor_buy: u32,
    pub vendor_sell: u32,
    /// Flat attack power contribution when equipped as a weapon.
    pub attack_power: f32,
    /// Flat armor when equipped in an armor slot.
    pub armor: f32,
    /// Equipment slot for weapons/armor; `None` for non-equippable.
    pub equip_slot: Option<ItemEquipSlot>,
    /// Minimum player level required to equip (default 1).
    pub level_req: u32,
    /// HP restored when used as a consumable (0 if not a heal).
    pub heal_hp: f32,
    pub armor_class: Option<ArmorClass>,
    pub weapon_style: Option<WeaponStyle>,
    pub allowed_classes: &'static [PlayerClass],
    pub stamina: f32,
    pub spell_power: f32,
}

pub fn class_armor_cap(class: PlayerClass) -> ArmorClass {
    match class {
        PlayerClass::Warrior | PlayerClass::Paladin => ArmorClass::Plate,
        PlayerClass::Hunter | PlayerClass::Shaman => ArmorClass::Mail,
        PlayerClass::Rogue | PlayerClass::Druid => ArmorClass::Leather,
        PlayerClass::Priest | PlayerClass::Mage | PlayerClass::Warlock => ArmorClass::Cloth,
    }
}

fn armor_rank(class: ArmorClass) -> u8 {
    match class {
        ArmorClass::Cloth => 0,
        ArmorClass::Leather => 1,
        ArmorClass::Mail => 2,
        ArmorClass::Plate => 3,
    }
}

pub fn can_equip(def: &ItemDef, class: PlayerClass, level: u32) -> Result<(), EquipDeny> {
    if def.equip_slot.is_none() {
        return Err(EquipDeny::NotGear);
    }
    if level < def.level_req {
        return Err(EquipDeny::LevelReq(def.level_req));
    }
    if !def.allowed_classes.is_empty() && !def.allowed_classes.contains(&class) {
        return Err(EquipDeny::WrongClass);
    }
    if let Some(ac) = def.armor_class {
        if armor_rank(ac) > armor_rank(class_armor_cap(class)) {
            return Err(EquipDeny::WrongArmor);
        }
    }
    Ok(())
}

const fn weapon(
    id: &'static str,
    name: &'static str,
    vendor_sell: u32,
    attack_power: f32,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell,
        attack_power,
        armor: 0.0,
        equip_slot: Some(ItemEquipSlot::MainHand),
        level_req: 1,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: Some(WeaponStyle::OneHand),
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
    }
}

const fn armor(
    id: &'static str,
    name: &'static str,
    slot: ItemEquipSlot,
    vendor_sell: u32,
    armor: f32,
    level_req: u32,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Armor,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell,
        attack_power: 0.0,
        armor,
        equip_slot: Some(slot),
        level_req,
        heal_hp: 0.0,
        armor_class: Some(ArmorClass::Cloth),
        weapon_style: None,
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
    }
}

const fn consumable(
    id: &'static str,
    name: &'static str,
    vendor_buy: u32,
    vendor_sell: u32,
    heal_hp: f32,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Consumable,
        stack_size: 20,
        vendor_buy,
        vendor_sell,
        attack_power: 0.0,
        armor: 0.0,
        equip_slot: None,
        level_req: 1,
        heal_hp,
        armor_class: None,
        weapon_style: None,
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
    }
}

const fn misc(id: &'static str, name: &'static str, kind: ItemKind, vendor_sell: u32) -> ItemDef {
    ItemDef {
        id,
        name,
        kind,
        stack_size: 20,
        vendor_buy: 0,
        vendor_sell,
        attack_power: 0.0,
        armor: 0.0,
        equip_slot: None,
        level_req: 1,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: None,
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
    }
}

pub static ZONE1_ITEMS: &[ItemDef] = &[
    weapon("worn_sword", "Worn Sword", 5, 8.0),
    weapon("worn_mace", "Worn Mace", 5, 7.0),
    weapon("worn_bow", "Worn Bow", 5, 7.0),
    weapon("worn_dagger", "Worn Dagger", 5, 6.0),
    weapon("worn_staff", "Worn Staff", 5, 5.0),
    armor(
        "recruit_tunic",
        "Recruit's Tunic",
        ItemEquipSlot::Chest,
        4,
        12.0,
        1,
    ),
    armor(
        "recruit_robe",
        "Recruit's Robe",
        ItemEquipSlot::Chest,
        4,
        6.0,
        1,
    ),
    armor(
        "recruit_cap",
        "Recruit's Cap",
        ItemEquipSlot::Head,
        3,
        4.0,
        1,
    ),
    armor(
        "recruit_pants",
        "Recruit's Pants",
        ItemEquipSlot::Legs,
        3,
        5.0,
        1,
    ),
    armor(
        "recruit_boots",
        "Recruit's Boots",
        ItemEquipSlot::Feet,
        3,
        3.0,
        1,
    ),
    armor(
        "wooden_buckler",
        "Wooden Buckler",
        ItemEquipSlot::OffHand,
        4,
        8.0,
        1,
    ),
    armor(
        "veteran_helm",
        "Veteran's Helm",
        ItemEquipSlot::Head,
        12,
        20.0,
        5,
    ),
    consumable("baked_bread", "Baked Bread", 5, 1, 40.0),
    consumable("spring_water", "Spring Water", 5, 1, 0.0),
    consumable("travelers_ration", "Traveler's Ration", 12, 3, 80.0),
    misc("wolf_fang", "Wolf Fang", ItemKind::Junk, 2),
    misc("boar_tusk", "Boar Tusk", ItemKind::Quest, 1),
    armor(
        "eastbrook_greaves",
        "Eastbrook Greaves",
        ItemEquipSlot::Legs,
        8,
        18.0,
        1,
    ),
    // Profession reagents (herbalism gather yields).
    misc("silverleaf", "Silverleaf", ItemKind::Junk, 1),
    misc("peacebloom", "Peacebloom", ItemKind::Junk, 1),
    misc("briarroot", "Briarroot", ItemKind::Junk, 2),
    // Alchemy craft products.
    consumable("minor_healing_salve", "Minor Healing Salve", 8, 2, 55.0),
    consumable("briar_tonic", "Briar Tonic", 10, 3, 35.0),
    // Mining / blacksmithing.
    misc("copper_ore", "Copper Ore", ItemKind::Junk, 1),
    misc("copper_bar", "Copper Bar", ItemKind::Junk, 2),
    weapon("copper_shortsword", "Copper Shortsword", 12, 11.0),
];

/// Zone1 + zone2 item definitions.
pub static ITEMS: LazyLock<&'static [ItemDef]> = LazyLock::new(|| {
    let mut all = Vec::with_capacity(ZONE1_ITEMS.len() + ZONE2_ITEMS.len());
    all.extend_from_slice(ZONE1_ITEMS);
    all.extend_from_slice(ZONE2_ITEMS);
    Box::leak(all.into_boxed_slice())
});

pub fn item(id: &str) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|i| i.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerClass;

    #[test]
    fn mage_cannot_wear_leather_or_sword() {
        let sword = item("worn_sword").unwrap();
        assert_eq!(
            can_equip(sword, PlayerClass::Mage, 10),
            Err(EquipDeny::WrongClass)
        );
        let tunic = item("recruit_tunic").unwrap();
        assert_eq!(
            can_equip(tunic, PlayerClass::Mage, 10),
            Err(EquipDeny::WrongArmor)
        );
    }

    #[test]
    fn warrior_can_wear_cloth_and_leather() {
        assert!(can_equip(item("recruit_robe").unwrap(), PlayerClass::Warrior, 1).is_ok());
        assert!(can_equip(item("recruit_tunic").unwrap(), PlayerClass::Warrior, 1).is_ok());
    }

    #[test]
    fn level_req_still_blocks() {
        assert_eq!(
            can_equip(item("veteran_helm").unwrap(), PlayerClass::Warrior, 1),
            Err(EquipDeny::LevelReq(5))
        );
    }

    #[test]
    fn junk_is_not_gear() {
        assert_eq!(
            can_equip(item("wolf_fang").unwrap(), PlayerClass::Warrior, 1),
            Err(EquipDeny::NotGear)
        );
    }
}
