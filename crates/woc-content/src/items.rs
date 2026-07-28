//! Item definitions for the framework slice.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Weapon,
    Armor,
    Consumable,
    Junk,
    Quest,
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
    /// Flat armor when equipped as chest.
    pub armor: f32,
}

pub static ITEMS: &[ItemDef] = &[
    ItemDef {
        id: "worn_sword",
        name: "Worn Sword",
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 5,
        attack_power: 8.0,
        armor: 0.0,
    },
    ItemDef {
        id: "worn_mace",
        name: "Worn Mace",
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 5,
        attack_power: 7.0,
        armor: 0.0,
    },
    ItemDef {
        id: "worn_bow",
        name: "Worn Bow",
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 5,
        attack_power: 7.0,
        armor: 0.0,
    },
    ItemDef {
        id: "worn_dagger",
        name: "Worn Dagger",
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 5,
        attack_power: 6.0,
        armor: 0.0,
    },
    ItemDef {
        id: "worn_staff",
        name: "Worn Staff",
        kind: ItemKind::Weapon,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 5,
        attack_power: 5.0,
        armor: 0.0,
    },
    ItemDef {
        id: "recruit_tunic",
        name: "Recruit's Tunic",
        kind: ItemKind::Armor,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 4,
        attack_power: 0.0,
        armor: 12.0,
    },
    ItemDef {
        id: "recruit_robe",
        name: "Recruit's Robe",
        kind: ItemKind::Armor,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 4,
        attack_power: 0.0,
        armor: 6.0,
    },
    ItemDef {
        id: "baked_bread",
        name: "Baked Bread",
        kind: ItemKind::Consumable,
        stack_size: 20,
        vendor_buy: 5,
        vendor_sell: 1,
        attack_power: 0.0,
        armor: 0.0,
    },
    ItemDef {
        id: "spring_water",
        name: "Spring Water",
        kind: ItemKind::Consumable,
        stack_size: 20,
        vendor_buy: 5,
        vendor_sell: 1,
        attack_power: 0.0,
        armor: 0.0,
    },
    ItemDef {
        id: "travelers_ration",
        name: "Traveler's Ration",
        kind: ItemKind::Consumable,
        stack_size: 20,
        vendor_buy: 12,
        vendor_sell: 3,
        attack_power: 0.0,
        armor: 0.0,
    },
    ItemDef {
        id: "wolf_fang",
        name: "Wolf Fang",
        kind: ItemKind::Junk,
        stack_size: 20,
        vendor_buy: 0,
        vendor_sell: 2,
        attack_power: 0.0,
        armor: 0.0,
    },
    ItemDef {
        id: "boar_tusk",
        name: "Boar Tusk",
        kind: ItemKind::Quest,
        stack_size: 20,
        vendor_buy: 0,
        vendor_sell: 1,
        attack_power: 0.0,
        armor: 0.0,
    },
    ItemDef {
        id: "eastbrook_greaves",
        name: "Eastbrook Greaves",
        kind: ItemKind::Armor,
        stack_size: 1,
        vendor_buy: 0,
        vendor_sell: 8,
        attack_power: 0.0,
        armor: 18.0,
    },
];

pub fn item(id: &str) -> Option<&'static ItemDef> {
    ITEMS.iter().find(|i| i.id == id)
}
