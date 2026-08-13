#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfessionCategory {
    Gathering,
    Crafting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ProfessionId {
    Mining,
    Herbalism,
    Skinning,
    Forging,
    Leatherworking,
    Tailoring,
    Jewelcrafting,
    Enchanting,
    Engineering,
    Alchemy,
}

impl ProfessionId {
    pub const ALL: [ProfessionId; 10] = [
        ProfessionId::Mining,
        ProfessionId::Herbalism,
        ProfessionId::Skinning,
        ProfessionId::Forging,
        ProfessionId::Leatherworking,
        ProfessionId::Tailoring,
        ProfessionId::Jewelcrafting,
        ProfessionId::Enchanting,
        ProfessionId::Engineering,
        ProfessionId::Alchemy,
    ];

    pub fn category(self) -> ProfessionCategory {
        match self {
            ProfessionId::Mining | ProfessionId::Herbalism | ProfessionId::Skinning => {
                ProfessionCategory::Gathering
            }
            _ => ProfessionCategory::Crafting,
        }
    }

    pub fn max_skill(self) -> u16 {
        match self.category() {
            ProfessionCategory::Gathering => 100,
            ProfessionCategory::Crafting => 125,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DenyReason {
    OutOfRange,
    NodeNotReady,
    MissingTool,
    ToolTierTooLow,
    InventoryFull,
    UnknownNode,
    Busy,
    CorpseGone,
    NothingToSkin,
    AlreadySkinned,
    MissingKnife,
    UnknownRecipe,
    MissingReagents,
    InsufficientGold,
    StationRequired,
    InvalidCount,
    UnknownEnchant,
    WrongSlot,
    AlreadyEnchanted,
    SameEnchant,
    NotInstanced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub z: f32,
}

impl Vec2 {
    pub fn distance(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        (dx * dx + dz * dz).sqrt()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Ore,
    Herb,
}

#[derive(Clone, Copy, Debug)]
pub struct GatherNodeDef {
    pub id: NodeId,
    pub kind: NodeKind,
    pub pos: Vec2,
    pub tier: u8,
    pub skill_req: u16,
    pub respawn_seconds: u32,
}

pub const TIER_SKILL_STEP: u16 = 25;
pub const HARVEST_RANGE: f32 = 5.0;
pub const STATION_RADIUS: f32 = 20.0;
pub const CRAFT_GOLD_SINK_COPPER_PER_BUDGET: u32 = 2;
pub const CRAFT_BATCH_MAX: u16 = 50;
