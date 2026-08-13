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
    Neck,
    Finger,
    Shoulder,
    Back,
    Wrist,
    Hands,
    Waist,
    Trinket,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemQuality {
    Poor,
    Common,
    Uncommon,
    Rare,
}

pub fn quality_mult(q: ItemQuality) -> f32 {
    match q {
        ItemQuality::Poor => 0.9,
        ItemQuality::Common => 1.0,
        ItemQuality::Uncommon => 1.1,
        ItemQuality::Rare => 1.2,
    }
}

impl ItemQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Poor => "poor",
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "poor" => Some(Self::Poor),
            "common" => Some(Self::Common),
            "uncommon" => Some(Self::Uncommon),
            "rare" => Some(Self::Rare),
            _ => None,
        }
    }
}

pub fn can_dual_wield(class: PlayerClass) -> bool {
    matches!(
        class,
        PlayerClass::Warrior | PlayerClass::Rogue | PlayerClass::Hunter
    )
}

#[derive(Debug, Clone, Copy)]
pub struct EnchantDef {
    pub id: &'static str,
    pub name: &'static str,
    pub attack_power: f32,
    pub stamina: f32,
    pub spell_power: f32,
}

pub static ENCHANTS: &[EnchantDef] = &[
    EnchantDef {
        id: "coarse_sharpening",
        name: "Coarse Sharpening",
        attack_power: 6.0,
        stamina: 0.0,
        spell_power: 0.0,
    },
    EnchantDef {
        id: "minor_wizard_oil",
        name: "Minor Wizard Oil",
        attack_power: 0.0,
        stamina: 0.0,
        spell_power: 6.0,
    },
];

pub fn enchant(id: &str) -> Option<&'static EnchantDef> {
    ENCHANTS.iter().find(|e| e.id == id)
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
    /// Maximum durability for gear; 0 means the item does not wear.
    pub max_durability: u32,
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
    pub quality: ItemQuality,
    pub enchant_id: Option<&'static str>,
}

const fn with_quality(mut def: ItemDef, quality: ItemQuality) -> ItemDef {
    def.quality = quality;
    def
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

const WARRIOR: &[PlayerClass] = &[PlayerClass::Warrior];
const PALADIN_SHAMAN: &[PlayerClass] = &[PlayerClass::Paladin, PlayerClass::Shaman];
const HUNTER: &[PlayerClass] = &[PlayerClass::Hunter];
const ROGUE: &[PlayerClass] = &[PlayerClass::Rogue];
const CASTERS: &[PlayerClass] = &[
    PlayerClass::Priest,
    PlayerClass::Mage,
    PlayerClass::Warlock,
    PlayerClass::Druid,
];
const WAR_PAL: &[PlayerClass] = &[PlayerClass::Warrior, PlayerClass::Paladin];
const WAR_HUN: &[PlayerClass] = &[PlayerClass::Warrior, PlayerClass::Hunter];
const WAR_PAL_ROGUE_HUN: &[PlayerClass] = &[
    PlayerClass::Warrior,
    PlayerClass::Paladin,
    PlayerClass::Rogue,
    PlayerClass::Hunter,
];
const WAR_PAL_SHA: &[PlayerClass] = &[
    PlayerClass::Warrior,
    PlayerClass::Paladin,
    PlayerClass::Shaman,
];

const fn weapon(
    id: &'static str,
    name: &'static str,
    vendor_buy: u32,
    vendor_sell: u32,
    attack_power: f32,
    style: WeaponStyle,
    allowed: &'static [PlayerClass],
) -> ItemDef {
    weapon_gear(
        id,
        name,
        vendor_buy,
        vendor_sell,
        attack_power,
        0.0,
        1,
        style,
        allowed,
    )
}

#[allow(clippy::too_many_arguments)]
const fn weapon_gear(
    id: &'static str,
    name: &'static str,
    vendor_buy: u32,
    vendor_sell: u32,
    attack_power: f32,
    spell_power: f32,
    level_req: u32,
    style: WeaponStyle,
    allowed: &'static [PlayerClass],
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Weapon,
        stack_size: 1,
        max_durability: 40,
        vendor_buy,
        vendor_sell,
        attack_power,
        armor: 0.0,
        equip_slot: Some(ItemEquipSlot::MainHand),
        level_req,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: Some(style),
        allowed_classes: allowed,
        stamina: 0.0,
        spell_power,
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}

#[allow(clippy::too_many_arguments)]
const fn jewelry(
    id: &'static str,
    name: &'static str,
    slot: ItemEquipSlot,
    vendor_buy: u32,
    vendor_sell: u32,
    stamina: f32,
    attack_power: f32,
    spell_power: f32,
    level_req: u32,
    allowed: &'static [PlayerClass],
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Armor,
        stack_size: 1,
        max_durability: 0,
        vendor_buy,
        vendor_sell,
        attack_power,
        armor: 0.0,
        equip_slot: Some(slot),
        level_req,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: None,
        allowed_classes: allowed,
        stamina,
        spell_power,
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}

#[allow(clippy::too_many_arguments)]
const fn armor(
    id: &'static str,
    name: &'static str,
    slot: ItemEquipSlot,
    vendor_buy: u32,
    vendor_sell: u32,
    armor: f32,
    level_req: u32,
    armor_class: ArmorClass,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Armor,
        stack_size: 1,
        max_durability: 30,
        vendor_buy,
        vendor_sell,
        attack_power: 0.0,
        armor,
        equip_slot: Some(slot),
        level_req,
        heal_hp: 0.0,
        armor_class: Some(armor_class),
        weapon_style: None,
        allowed_classes: &[],
        stamina: 0.0,
        spell_power: 0.0,
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}

const fn shield(
    id: &'static str,
    name: &'static str,
    vendor_buy: u32,
    vendor_sell: u32,
    armor: f32,
    allowed: &'static [PlayerClass],
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind: ItemKind::Armor,
        stack_size: 1,
        max_durability: 30,
        vendor_buy,
        vendor_sell,
        attack_power: 0.0,
        armor,
        equip_slot: Some(ItemEquipSlot::OffHand),
        level_req: 1,
        heal_hp: 0.0,
        armor_class: None,
        weapon_style: Some(WeaponStyle::Shield),
        allowed_classes: allowed,
        stamina: 0.0,
        spell_power: 0.0,
        quality: ItemQuality::Common,
        enchant_id: None,
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
        max_durability: 0,
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
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}

const fn misc(id: &'static str, name: &'static str, kind: ItemKind, vendor_sell: u32) -> ItemDef {
    ItemDef {
        id,
        name,
        kind,
        stack_size: 20,
        max_durability: 0,
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
        quality: ItemQuality::Common,
        enchant_id: None,
    }
}

const fn enchant_oil(id: &'static str, name: &'static str, enchant_id: &'static str) -> ItemDef {
    let mut def = consumable(id, name, 15, 3, 0.0);
    def.stack_size = 5;
    def.enchant_id = Some(enchant_id);
    def
}

pub static ZONE1_ITEMS: &[ItemDef] = &[
    weapon(
        "worn_sword",
        "Worn Sword",
        20,
        5,
        8.0,
        WeaponStyle::OneHand,
        WARRIOR,
    ),
    weapon(
        "worn_mace",
        "Worn Mace",
        0,
        5,
        7.0,
        WeaponStyle::OneHand,
        PALADIN_SHAMAN,
    ),
    weapon(
        "worn_bow",
        "Worn Bow",
        0,
        5,
        7.0,
        WeaponStyle::Ranged,
        HUNTER,
    ),
    weapon(
        "worn_hatchet",
        "Worn Hatchet",
        18,
        5,
        7.0,
        WeaponStyle::OneHand,
        WAR_HUN,
    ),
    weapon(
        "worn_dagger",
        "Worn Dagger",
        0,
        5,
        6.0,
        WeaponStyle::OneHand,
        ROGUE,
    ),
    weapon(
        "worn_staff",
        "Worn Staff",
        0,
        5,
        5.0,
        WeaponStyle::TwoHand,
        CASTERS,
    ),
    armor(
        "recruit_tunic",
        "Recruit's Tunic",
        ItemEquipSlot::Chest,
        16,
        4,
        12.0,
        1,
        ArmorClass::Leather,
    ),
    armor(
        "recruit_robe",
        "Recruit's Robe",
        ItemEquipSlot::Chest,
        0,
        4,
        6.0,
        1,
        ArmorClass::Cloth,
    ),
    armor(
        "recruit_cap",
        "Recruit's Cap",
        ItemEquipSlot::Head,
        0,
        3,
        4.0,
        1,
        ArmorClass::Cloth,
    ),
    armor(
        "recruit_pants",
        "Recruit's Pants",
        ItemEquipSlot::Legs,
        0,
        3,
        5.0,
        1,
        ArmorClass::Cloth,
    ),
    armor(
        "recruit_boots",
        "Recruit's Boots",
        ItemEquipSlot::Feet,
        0,
        3,
        3.0,
        1,
        ArmorClass::Cloth,
    ),
    shield("wooden_buckler", "Wooden Buckler", 16, 4, 8.0, WAR_PAL_SHA),
    armor(
        "padded_shoulders",
        "Padded Shoulders",
        ItemEquipSlot::Shoulder,
        14,
        4,
        4.0,
        1,
        ArmorClass::Cloth,
    ),
    with_quality(
        armor(
            "wool_cloak",
            "Wool Cloak",
            ItemEquipSlot::Back,
            22,
            6,
            3.0,
            1,
            ArmorClass::Cloth,
        ),
        ItemQuality::Uncommon,
    ),
    armor(
        "frayed_cuffs",
        "Frayed Cuffs",
        ItemEquipSlot::Wrist,
        8,
        2,
        2.0,
        1,
        ArmorClass::Cloth,
    ),
    with_quality(
        armor(
            "work_gloves",
            "Work Gloves",
            ItemEquipSlot::Hands,
            18,
            5,
            3.0,
            1,
            ArmorClass::Leather,
        ),
        ItemQuality::Uncommon,
    ),
    armor(
        "frayed_belt",
        "Frayed Belt",
        ItemEquipSlot::Waist,
        8,
        2,
        2.0,
        1,
        ArmorClass::Cloth,
    ),
    jewelry(
        "lucky_pebble",
        "Lucky Pebble",
        ItemEquipSlot::Trinket,
        12,
        3,
        2.0,
        0.0,
        0.0,
        1,
        &[],
    ),
    with_quality(
        armor(
            "veteran_helm",
            "Veteran's Helm",
            ItemEquipSlot::Head,
            0,
            12,
            20.0,
            5,
            ArmorClass::Mail,
        ),
        ItemQuality::Uncommon,
    ),
    consumable("baked_bread", "Baked Bread", 5, 1, 40.0),
    consumable("spring_water", "Spring Water", 5, 1, 0.0),
    consumable("travelers_ration", "Traveler's Ration", 12, 3, 80.0),
    misc("wolf_fang", "Wolf Fang", ItemKind::Junk, 2),
    misc("boar_tusk", "Boar Tusk", ItemKind::Quest, 1),
    with_quality(
        armor(
            "eastbrook_greaves",
            "Eastbrook Greaves",
            ItemEquipSlot::Legs,
            0,
            8,
            18.0,
            1,
            ArmorClass::Leather,
        ),
        ItemQuality::Uncommon,
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
    with_quality(
        weapon(
            "copper_shortsword",
            "Copper Shortsword",
            48,
            12,
            11.0,
            WeaponStyle::OneHand,
            WAR_PAL_ROGUE_HUN,
        ),
        ItemQuality::Uncommon,
    ),
    with_quality(
        jewelry(
            "fang_pendant",
            "Fang Pendant",
            ItemEquipSlot::Neck,
            40,
            10,
            4.0,
            0.0,
            0.0,
            1,
            &[],
        ),
        ItemQuality::Uncommon,
    ),
    with_quality(
        jewelry(
            "boar_tusk_ring",
            "Boar Tusk Ring",
            ItemEquipSlot::Finger,
            48,
            12,
            3.0,
            1.0,
            0.0,
            1,
            &[],
        ),
        ItemQuality::Uncommon,
    ),
    with_quality(
        weapon_gear(
            "crypt_cleaver",
            "Crypt Cleaver",
            96,
            24,
            16.0,
            0.0,
            3,
            WeaponStyle::TwoHand,
            WAR_PAL,
        ),
        ItemQuality::Rare,
    ),
    enchant_oil("coarse_whetstone", "Coarse Whetstone", "coarse_sharpening"),
    enchant_oil("minor_wizard_oil", "Minor Wizard Oil", "minor_wizard_oil"),
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

    #[test]
    fn dual_wield_classes() {
        assert!(can_dual_wield(PlayerClass::Warrior));
        assert!(can_dual_wield(PlayerClass::Rogue));
        assert!(can_dual_wield(PlayerClass::Hunter));
        assert!(!can_dual_wield(PlayerClass::Mage));
        assert!(!can_dual_wield(PlayerClass::Shaman));
    }

    #[test]
    fn extra_slots_and_hatchet_exist() {
        assert_eq!(
            item("wool_cloak").unwrap().equip_slot,
            Some(ItemEquipSlot::Back)
        );
        assert_eq!(
            item("lucky_pebble").unwrap().equip_slot,
            Some(ItemEquipSlot::Trinket)
        );
        let hatchet = item("worn_hatchet").unwrap();
        assert_eq!(hatchet.weapon_style, Some(WeaponStyle::OneHand));
        assert_eq!(
            can_equip(hatchet, PlayerClass::Hunter, 1),
            Ok(())
        );
        assert_eq!(
            can_equip(item("copper_shortsword").unwrap(), PlayerClass::Hunter, 1),
            Ok(())
        );
    }

    #[test]
    fn rare_hag_focus_and_oils() {
        assert_eq!(item("hag_focus").unwrap().quality, ItemQuality::Rare);
        assert_eq!(quality_mult(ItemQuality::Rare), 1.2);
        assert_eq!(
            item("coarse_whetstone").unwrap().enchant_id,
            Some("coarse_sharpening")
        );
        assert!(enchant("coarse_sharpening").is_some());
    }
}
