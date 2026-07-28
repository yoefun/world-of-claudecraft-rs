//! Item definitions for the framework slice.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::items_zone2::ZONE2_ITEMS;

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
    }
}

const fn misc(
    id: &'static str,
    name: &'static str,
    kind: ItemKind,
    vendor_sell: u32,
) -> ItemDef {
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
