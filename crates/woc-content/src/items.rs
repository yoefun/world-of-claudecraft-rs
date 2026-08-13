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
    /// Reserved for future bracers; v1 ships no wrist gear.
    Wrist,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

pub fn can_dual_wield(class: PlayerClass) -> bool {
    matches!(class, PlayerClass::Warrior | PlayerClass::Rogue)
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
    EnchantDef {
        id: "weapon_minor_might",
        name: "Weapon Minor Might",
        attack_power: 2.0,
        stamina: 0.0,
        spell_power: 0.0,
    },
    EnchantDef {
        id: "chest_minor_stamina",
        name: "Chest Minor Stamina",
        attack_power: 0.0,
        stamina: 3.0,
        spell_power: 0.0,
    },
    EnchantDef {
        id: "bracer_minor_health",
        name: "Bracer Minor Health",
        attack_power: 0.0,
        stamina: 2.0,
        spell_power: 0.0,
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
const WAR_PAL_ROGUE: &[PlayerClass] = &[
    PlayerClass::Warrior,
    PlayerClass::Paladin,
    PlayerClass::Rogue,
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
    valued_misc(id, name, kind, vendor_sell, 0, 20)
}

const fn gathered(id: &'static str, name: &'static str, vendor_sell: u32) -> ItemDef {
    valued_misc(id, name, ItemKind::Junk, vendor_sell, vendor_sell.saturating_mul(4), 20)
}

const fn vendor_mat(id: &'static str, name: &'static str, vendor_sell: u32) -> ItemDef {
    valued_misc(id, name, ItemKind::Junk, vendor_sell, vendor_sell.saturating_mul(4), 20)
}

const fn tool(id: &'static str, name: &'static str, vendor_sell: u32) -> ItemDef {
    valued_misc(id, name, ItemKind::Junk, vendor_sell, vendor_sell.saturating_mul(4), 1)
}

const fn valued_misc(
    id: &'static str,
    name: &'static str,
    kind: ItemKind,
    vendor_sell: u32,
    vendor_buy: u32,
    stack_size: u32,
) -> ItemDef {
    ItemDef {
        id,
        name,
        kind,
        stack_size,
        max_durability: 0,
        vendor_buy,
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
    // Profession reagents (herbalism / mining / skinning yields).
    gathered("silverleaf", "Silverleaf", 5),
    gathered("fine_silverleaf", "Fine Silverleaf", 12),
    gathered("peacebloom", "Peacebloom", 5),
    gathered("briarroot", "Briarroot", 6),
    gathered("earthroot", "Earthroot", 6),
    gathered("fine_earthroot", "Fine Earthroot", 14),
    gathered("copper_ore", "Copper Ore", 5),
    gathered("fine_copper_ore", "Fine Copper Ore", 12),
    gathered("coarse_stone", "Coarse Stone", 2),
    gathered("light_leather", "Light Leather", 8),
    gathered("fine_light_leather", "Fine Light Leather", 18),
    // Alchemy craft products.
    consumable("minor_healing_salve", "Minor Healing Salve", 8, 2, 55.0),
    consumable("briar_tonic", "Briar Tonic", 10, 3, 35.0),
    consumable("minor_healing_potion", "Minor Healing Potion", 0, 12, 80.0),
    consumable("elixir_of_minor_strength", "Elixir of Minor Strength", 0, 14, 0.0),
    // Mining / blacksmithing / manufacturing products.
    misc("copper_bar", "Copper Bar", ItemKind::Junk, 8),
    vendor_mat("smithing_flux", "Smithing Flux", 4),
    tool("copper_pick", "Copper Pick", 20),
    tool("copper_sickle", "Copper Sickle", 20),
    tool("skinning_knife", "Skinning Knife", 15),
    with_quality(
        weapon(
            "copper_shortsword",
            "Copper Shortsword",
            48,
            28,
            11.0,
            WeaponStyle::OneHand,
            WAR_PAL_ROGUE,
        ),
        ItemQuality::Uncommon,
    ),
    armor(
        "copper_chain_vest",
        "Copper Chain Vest",
        ItemEquipSlot::Chest,
        0,
        40,
        22.0,
        1,
        ArmorClass::Mail,
    ),
    misc("cured_light_leather", "Cured Light Leather", ItemKind::Junk, 10),
    armor(
        "light_leather_jerkin",
        "Light Leather Jerkin",
        ItemEquipSlot::Chest,
        0,
        36,
        12.0,
        1,
        ArmorClass::Leather,
    ),
    armor(
        "light_leather_belt",
        "Light Leather Belt",
        ItemEquipSlot::Legs,
        0,
        16,
        6.0,
        1,
        ArmorClass::Leather,
    ),
    vendor_mat("spool_of_thread", "Spool of Thread", 3),
    vendor_mat("empty_vial", "Empty Vial", 2),
    vendor_mat("linen_cloth", "Linen Cloth", 4),
    misc("bolt_of_linen", "Bolt of Linen", ItemKind::Junk, 6),
    armor(
        "linen_trousers",
        "Linen Trousers",
        ItemEquipSlot::Legs,
        0,
        40,
        8.0,
        1,
        ArmorClass::Cloth,
    ),
    armor(
        "linen_vestments",
        "Linen Vestments",
        ItemEquipSlot::Chest,
        0,
        50,
        10.0,
        1,
        ArmorClass::Cloth,
    ),
    misc("tigerseye", "Tigerseye", ItemKind::Junk, 15),
    misc("copper_setting", "Copper Setting", ItemKind::Junk, 6),
    jewelry(
        "tigerseye_band",
        "Tigerseye Band",
        ItemEquipSlot::Finger,
        0,
        18,
        1.0,
        0.0,
        0.0,
        1,
        &[],
    ),
    misc("rough_blasting_powder", "Rough Blasting Powder", ItemKind::Junk, 3),
    misc("copper_bolt", "Copper Bolt", ItemKind::Junk, 3),
    misc("copper_grenade", "Copper Grenade", ItemKind::Junk, 8),
    misc("arcane_dust", "Arcane Dust", ItemKind::Junk, 6),
    misc("arcane_essence", "Arcane Essence", ItemKind::Junk, 20),
    misc("arcane_shard", "Arcane Shard", ItemKind::Junk, 80),
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

pub fn reagent_unit_value(def: &ItemDef) -> u32 {
    if def.vendor_buy > 0 {
        def.vendor_buy
    } else {
        def.vendor_sell
    }
}

pub fn item_is_gathered(id: &str) -> bool {
    matches!(
        id,
        "copper_ore"
            | "fine_copper_ore"
            | "coarse_stone"
            | "silverleaf"
            | "fine_silverleaf"
            | "peacebloom"
            | "briarroot"
            | "earthroot"
            | "fine_earthroot"
            | "light_leather"
            | "fine_light_leather"
    )
}

/// Fine materials substitute downward for the matching base reagent.
pub fn fine_substitute_for(base_item_id: &str) -> Option<&'static str> {
    match base_item_id {
        "copper_ore" => Some("fine_copper_ore"),
        "silverleaf" => Some("fine_silverleaf"),
        "earthroot" => Some("fine_earthroot"),
        "light_leather" => Some("fine_light_leather"),
        _ => None,
    }
}

pub fn base_of(item_id: &str) -> &str {
    match item_id {
        "fine_copper_ore" => "copper_ore",
        "fine_silverleaf" => "silverleaf",
        "fine_earthroot" => "earthroot",
        "fine_light_leather" => "light_leather",
        other => other,
    }
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
        assert!(!can_dual_wield(PlayerClass::Mage));
        assert!(!can_dual_wield(PlayerClass::Hunter));
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
