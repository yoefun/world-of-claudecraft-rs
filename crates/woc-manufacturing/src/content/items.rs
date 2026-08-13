use crate::item::{EquipSlot, ItemDef, ItemId, Quality};

pub const ITEM_DEFS: &[ItemDef] = &[
    ItemDef { id: ItemId::CopperOre, quality: Quality::Common, slot: EquipSlot::None, sell_value: 5, buy_value: 20, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineCopperOre, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 12, buy_value: 48, stackable: true, gathered: true },
    ItemDef { id: ItemId::CoarseStone, quality: Quality::Common, slot: EquipSlot::None, sell_value: 2, buy_value: 8, stackable: true, gathered: true },
    ItemDef { id: ItemId::Silverleaf, quality: Quality::Common, slot: EquipSlot::None, sell_value: 5, buy_value: 20, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineSilverleaf, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 12, buy_value: 48, stackable: true, gathered: true },
    ItemDef { id: ItemId::Earthroot, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 24, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineEarthroot, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 14, buy_value: 56, stackable: true, gathered: true },
    ItemDef { id: ItemId::LightLeather, quality: Quality::Common, slot: EquipSlot::None, sell_value: 8, buy_value: 32, stackable: true, gathered: true },
    ItemDef { id: ItemId::FineLightLeather, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 18, buy_value: 72, stackable: true, gathered: true },
    ItemDef { id: ItemId::CuredLightLeather, quality: Quality::Common, slot: EquipSlot::None, sell_value: 10, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperBar, quality: Quality::Common, slot: EquipSlot::None, sell_value: 8, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::SmithingFlux, quality: Quality::Common, slot: EquipSlot::None, sell_value: 4, buy_value: 16, stackable: true, gathered: false },
    ItemDef { id: ItemId::SpoolOfThread, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 12, stackable: true, gathered: false },
    ItemDef { id: ItemId::EmptyVial, quality: Quality::Common, slot: EquipSlot::None, sell_value: 2, buy_value: 8, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperPick, quality: Quality::Common, slot: EquipSlot::None, sell_value: 20, buy_value: 80, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperSickle, quality: Quality::Common, slot: EquipSlot::None, sell_value: 20, buy_value: 80, stackable: false, gathered: false },
    ItemDef { id: ItemId::SkinningKnife, quality: Quality::Common, slot: EquipSlot::None, sell_value: 15, buy_value: 60, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperShortsword, quality: Quality::Common, slot: EquipSlot::MainHand, sell_value: 28, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::CopperChainVest, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 40, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LightLeatherJerkin, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 36, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LightLeatherBelt, quality: Quality::Common, slot: EquipSlot::Waist, sell_value: 16, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LinenCloth, quality: Quality::Common, slot: EquipSlot::None, sell_value: 4, buy_value: 16, stackable: true, gathered: false },
    ItemDef { id: ItemId::BoltOfLinen, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::LinenTrousers, quality: Quality::Common, slot: EquipSlot::Legs, sell_value: 40, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::LinenVestments, quality: Quality::Common, slot: EquipSlot::Chest, sell_value: 50, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::Tigerseye, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 15, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperSetting, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::TigerseyeBand, quality: Quality::Common, slot: EquipSlot::Ring, sell_value: 18, buy_value: 0, stackable: false, gathered: false },
    ItemDef { id: ItemId::MinorHealingPotion, quality: Quality::Common, slot: EquipSlot::None, sell_value: 12, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ElixirOfMinorStrength, quality: Quality::Common, slot: EquipSlot::None, sell_value: 14, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::RoughBlastingPowder, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperBolt, quality: Quality::Common, slot: EquipSlot::None, sell_value: 3, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::CopperGrenade, quality: Quality::Common, slot: EquipSlot::None, sell_value: 8, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneDust, quality: Quality::Common, slot: EquipSlot::None, sell_value: 6, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneEssence, quality: Quality::Uncommon, slot: EquipSlot::None, sell_value: 20, buy_value: 0, stackable: true, gathered: false },
    ItemDef { id: ItemId::ArcaneShard, quality: Quality::Rare, slot: EquipSlot::None, sell_value: 80, buy_value: 0, stackable: true, gathered: false },
];

pub fn item_def(id: ItemId) -> &'static ItemDef {
    ITEM_DEFS
        .iter()
        .find(|d| d.id == id)
        .expect("missing ItemDef")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemId;

    #[test]
    fn every_item_id_has_exactly_one_def() {
        let ids = [
            ItemId::CopperOre,
            ItemId::FineCopperOre,
            ItemId::CoarseStone,
            ItemId::Silverleaf,
            ItemId::FineSilverleaf,
            ItemId::Earthroot,
            ItemId::FineEarthroot,
            ItemId::LightLeather,
            ItemId::FineLightLeather,
            ItemId::CuredLightLeather,
            ItemId::CopperBar,
            ItemId::SmithingFlux,
            ItemId::SpoolOfThread,
            ItemId::EmptyVial,
            ItemId::CopperPick,
            ItemId::CopperSickle,
            ItemId::SkinningKnife,
            ItemId::CopperShortsword,
            ItemId::CopperChainVest,
            ItemId::LightLeatherJerkin,
            ItemId::LightLeatherBelt,
            ItemId::LinenCloth,
            ItemId::BoltOfLinen,
            ItemId::LinenTrousers,
            ItemId::LinenVestments,
            ItemId::Tigerseye,
            ItemId::CopperSetting,
            ItemId::TigerseyeBand,
            ItemId::MinorHealingPotion,
            ItemId::ElixirOfMinorStrength,
            ItemId::RoughBlastingPowder,
            ItemId::CopperBolt,
            ItemId::CopperGrenade,
            ItemId::ArcaneDust,
            ItemId::ArcaneEssence,
            ItemId::ArcaneShard,
        ];
        assert_eq!(ids.len(), ITEM_DEFS.len());
        for id in ids {
            assert_eq!(item_def(id).id, id);
        }
    }

    #[test]
    fn gathered_materials_use_four_times_buy_value() {
        for def in ITEM_DEFS.iter().filter(|d| d.gathered) {
            assert_eq!(def.buy_value, def.sell_value * 4, "{:?} buy/sell ratio", def.id);
        }
    }
}
