use crate::item::{EquipSlot, ItemId, Quality};
use crate::professions::types::Reagent;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EnchantId {
    BracerMinorHealth,
    WeaponMinorMight,
    ChestMinorStamina,
}

#[derive(Clone, Copy, Debug)]
pub struct EnchantDef {
    pub id: EnchantId,
    pub slot: EquipSlot,
    pub reagents: &'static [Reagent],
    pub sta: u8,
    pub str: u8,
}

const BRACER_MINOR_HEALTH_REAGENTS: &[Reagent] = &[Reagent {
    item: ItemId::ArcaneDust,
    count: 2,
}];

const WEAPON_MINOR_MIGHT_REAGENTS: &[Reagent] = &[Reagent {
    item: ItemId::ArcaneDust,
    count: 5,
}];

const CHEST_MINOR_STAMINA_REAGENTS: &[Reagent] = &[
    Reagent {
        item: ItemId::ArcaneDust,
        count: 3,
    },
    Reagent {
        item: ItemId::ArcaneEssence,
        count: 1,
    },
];

pub const ENCHANT_DEFS: &[EnchantDef] = &[
    EnchantDef {
        id: EnchantId::BracerMinorHealth,
        slot: EquipSlot::Wrist,
        reagents: BRACER_MINOR_HEALTH_REAGENTS,
        sta: 2,
        str: 0,
    },
    EnchantDef {
        id: EnchantId::WeaponMinorMight,
        slot: EquipSlot::MainHand,
        reagents: WEAPON_MINOR_MIGHT_REAGENTS,
        sta: 0,
        str: 2,
    },
    EnchantDef {
        id: EnchantId::ChestMinorStamina,
        slot: EquipSlot::Chest,
        reagents: CHEST_MINOR_STAMINA_REAGENTS,
        sta: 3,
        str: 0,
    },
];

pub fn enchant_by_id(id: EnchantId) -> &'static EnchantDef {
    ENCHANT_DEFS
        .iter()
        .find(|e| e.id == id)
        .expect("missing EnchantDef")
}

pub fn disenchant_yield(quality: Quality) -> &'static [Reagent] {
    match quality {
        Quality::Common => &[Reagent {
            item: ItemId::ArcaneDust,
            count: 1,
        }],
        Quality::Uncommon => &[Reagent {
            item: ItemId::ArcaneDust,
            count: 2,
        }],
        Quality::Rare => &[
            Reagent {
                item: ItemId::ArcaneDust,
                count: 2,
            },
            Reagent {
                item: ItemId::ArcaneEssence,
                count: 1,
            },
        ],
        Quality::Epic => &[Reagent {
            item: ItemId::ArcaneShard,
            count: 1,
        }],
    }
}
