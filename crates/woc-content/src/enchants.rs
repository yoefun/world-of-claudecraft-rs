//! Profession enchant recipes and disenchant yields.
//!
//! Gear enchant *stats* live in [`crate::items::ENCHANTS`]. This module owns
//! the craftable profession recipes (reagents + target slot).

use crate::items::ItemEquipSlot;

#[derive(Debug, Clone, Copy)]
pub struct EnchantReagent {
    pub item_id: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfessionEnchantDef {
    pub id: &'static str,
    pub slot: ItemEquipSlot,
    pub reagents: &'static [EnchantReagent],
    pub stamina: u32,
    pub attack_power: u32,
}

const BRACER_MINOR_HEALTH: &[EnchantReagent] = &[EnchantReagent {
    item_id: "arcane_dust",
    count: 2,
}];

const WEAPON_MINOR_MIGHT: &[EnchantReagent] = &[EnchantReagent {
    item_id: "arcane_dust",
    count: 5,
}];

const CHEST_MINOR_STAMINA: &[EnchantReagent] = &[
    EnchantReagent {
        item_id: "arcane_dust",
        count: 3,
    },
    EnchantReagent {
        item_id: "arcane_essence",
        count: 1,
    },
];

pub static PROFESSION_ENCHANTS: &[ProfessionEnchantDef] = &[
    ProfessionEnchantDef {
        id: "bracer_minor_health",
        slot: ItemEquipSlot::Wrist,
        reagents: BRACER_MINOR_HEALTH,
        stamina: 2,
        attack_power: 0,
    },
    ProfessionEnchantDef {
        id: "weapon_minor_might",
        slot: ItemEquipSlot::MainHand,
        reagents: WEAPON_MINOR_MIGHT,
        stamina: 0,
        attack_power: 2,
    },
    ProfessionEnchantDef {
        id: "chest_minor_stamina",
        slot: ItemEquipSlot::Chest,
        reagents: CHEST_MINOR_STAMINA,
        stamina: 3,
        attack_power: 0,
    },
];

pub fn profession_enchant(id: &str) -> Option<&'static ProfessionEnchantDef> {
    PROFESSION_ENCHANTS.iter().find(|e| e.id == id)
}

/// Common / uncommon / rare / epic yields. Gear without a `fine_` prefix is common.
pub fn disenchant_yield(item_id: &str) -> &'static [EnchantReagent] {
    if item_id == "arcane_shard" || item_id.contains("epic") {
        return &[EnchantReagent {
            item_id: "arcane_shard",
            count: 1,
        }];
    }
    if item_id.starts_with("fine_") || item_id == "tigerseye" || item_id == "arcane_essence" {
        return &[EnchantReagent {
            item_id: "arcane_dust",
            count: 2,
        }];
    }
    if item_id == "crypt_cleaver" {
        return &[
            EnchantReagent {
                item_id: "arcane_dust",
                count: 2,
            },
            EnchantReagent {
                item_id: "arcane_essence",
                count: 1,
            },
        ];
    }
    &[EnchantReagent {
        item_id: "arcane_dust",
        count: 1,
    }]
}
