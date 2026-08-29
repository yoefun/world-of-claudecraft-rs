#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Quality {
    Common,
    Uncommon,
    Rare,
    Epic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EquipSlot {
    MainHand,
    Chest,
    Wrist,
    Waist,
    Legs,
    Ring,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ItemId {
    CopperOre,
    FineCopperOre,
    CoarseStone,
    Silverleaf,
    FineSilverleaf,
    Earthroot,
    FineEarthroot,
    LightLeather,
    FineLightLeather,
    CuredLightLeather,
    CopperBar,
    SmithingFlux,
    SpoolOfThread,
    EmptyVial,
    CopperPick,
    CopperSickle,
    SkinningKnife,
    CopperShortsword,
    CopperChainVest,
    LightLeatherJerkin,
    LightLeatherBelt,
    LinenCloth,
    BoltOfLinen,
    LinenTrousers,
    LinenVestments,
    Tigerseye,
    CopperSetting,
    TigerseyeBand,
    MinorHealingPotion,
    ElixirOfMinorStrength,
    RoughBlastingPowder,
    CopperBolt,
    CopperGrenade,
    ArcaneDust,
    ArcaneEssence,
    ArcaneShard,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub id: ItemId,
    pub quality: Quality,
    pub slot: EquipSlot,
    pub sell_value: u32,
    pub buy_value: u32,
    pub stackable: bool,
    pub gathered: bool,
}

pub fn reagent_unit_value(def: &ItemDef) -> u32 {
    if def.buy_value > 0 {
        def.buy_value
    } else {
        def.sell_value
    }
}
