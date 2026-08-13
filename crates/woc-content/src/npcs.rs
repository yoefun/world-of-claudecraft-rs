//! NPC definitions.

use std::sync::LazyLock;

use crate::factions::Standing;
use crate::npcs_zone2::ZONE2_NPCS;
use crate::npcs_zone3::ZONE3_NPCS;

#[derive(Debug, Clone)]
pub struct VendorOffer {
    pub item_id: &'static str,
    pub count: u32,
    pub min_standing: Standing,
}

impl VendorOffer {
    pub const fn stack(item_id: &'static str, count: u32) -> Self {
        Self {
            item_id,
            count,
            min_standing: Standing::Neutral,
        }
    }

    pub const fn gated(item_id: &'static str, count: u32, min_standing: Standing) -> Self {
        Self {
            item_id,
            count,
            min_standing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcService {
    QuestGiver,
    Vendor,
    Repair,
    ProfessionTrainer,
    ClassTrainer,
    Innkeeper,
    RidingTrainer,
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub id: &'static str,
    pub name: &'static str,
    pub greeting: &'static str,
    pub services: &'static [NpcService],
    pub vendor_stock: &'static [VendorOffer],
    pub trains: &'static [&'static str],
    pub faction: Option<&'static str>,
}

impl NpcDef {
    pub fn is_quest_giver(&self) -> bool {
        self.services.contains(&NpcService::QuestGiver)
    }

    pub fn is_vendor(&self) -> bool {
        self.services.contains(&NpcService::Vendor)
    }

    pub fn can_repair(&self) -> bool {
        self.services.contains(&NpcService::Repair)
    }

    pub fn is_profession_trainer(&self) -> bool {
        self.services.contains(&NpcService::ProfessionTrainer)
    }

    pub fn is_class_trainer(&self) -> bool {
        self.services.contains(&NpcService::ClassTrainer)
    }

    pub fn is_innkeeper(&self) -> bool {
        self.services.contains(&NpcService::Innkeeper)
    }

    pub fn is_riding_trainer(&self) -> bool {
        self.services.contains(&NpcService::RidingTrainer)
    }

    pub fn trains_profession(&self, id: &str) -> bool {
        self.trains.contains(&id)
    }
}

pub static ZONE1_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "captain_alden",
        name: "Captain Alden",
        greeting: "The north road is thick with wolves. Can you thin the pack?",
        services: &[NpcService::QuestGiver, NpcService::ClassTrainer],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "trader_wilkes",
        name: "Trader Wilkes",
        greeting: "Fresh rations and a fair price, traveler.",
        services: &[
            NpcService::QuestGiver,
            NpcService::Vendor,
            NpcService::ProfessionTrainer,
        ],
        vendor_stock: &[
            VendorOffer::stack("travelers_ration", 20),
            VendorOffer::stack("baked_bread", 40),
            VendorOffer::stack("spring_water", 40),
            VendorOffer::stack("spool_of_thread", 20),
            VendorOffer::stack("linen_cloth", 20),
            VendorOffer::gated("watch_signet", 1, Standing::Friendly),
            VendorOffer::stack("worn_hatchet", 2),
            VendorOffer::stack("lucky_pebble", 2),
        ],
        trains: &["skinning", "leatherworking", "tailoring", "jewelcrafting"],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "town_crier",
        name: "Town Crier",
        greeting: "Hear ye! Eastbrook stands, and the Vale endures.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "eastbrook_courier",
        name: "Eastbrook Courier",
        greeting: "Stay close — the north road is not kind to messengers.",
        services: &[],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "smith_brann",
        name: "Smith Brann",
        greeting: "Steel and ore. I can mend what the road breaks.",
        services: &[
            NpcService::Vendor,
            NpcService::Repair,
            NpcService::ProfessionTrainer,
        ],
        vendor_stock: &[
            VendorOffer::stack("worn_sword", 1),
            VendorOffer::stack("wooden_buckler", 1),
            VendorOffer::stack("copper_shortsword", 1),
            VendorOffer::stack("recruit_tunic", 1),
            VendorOffer::stack("coarse_whetstone", 20),
            VendorOffer::stack("minor_wizard_oil", 20),
            VendorOffer::stack("copper_pick", 5),
            VendorOffer::stack("skinning_knife", 5),
            VendorOffer::stack("smithing_flux", 20),
            VendorOffer::stack("copper_sickle", 5),
            VendorOffer::stack("empty_vial", 20),
            VendorOffer::stack("wool_cloak", 8),
            VendorOffer::stack("work_gloves", 8),
        ],
        trains: &["mining", "blacksmithing", "engineering"],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "herbalist_wren",
        name: "Herbalist Wren",
        greeting: "The vale still grows, if you know where to kneel.",
        services: &[NpcService::ProfessionTrainer],
        vendor_stock: &[],
        trains: &["herbalism", "alchemy", "enchanting"],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "innkeeper_mara",
        name: "Innkeeper Mara",
        greeting: "Rest the night. I'll keep the hearth.",
        services: &[NpcService::Innkeeper],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastbrook_watch"),
    },
    NpcDef {
        id: "stable_master_ross",
        name: "Stable Master Ross",
        greeting: "A horse knows the road better than most maps.",
        services: &[NpcService::RidingTrainer, NpcService::Vendor],
        vendor_stock: &[
            VendorOffer::stack("brown_pony", 1),
            VendorOffer::stack("swift_bay_steed", 1),
            VendorOffer::stack("tawny_gryphon", 1),
        ],
        trains: &[],
        faction: Some("eastbrook_watch"),
    },
];

/// Zone1 + zone2 + zone3 NPC definitions.
pub static NPCS: LazyLock<&'static [NpcDef]> = LazyLock::new(|| {
    let mut all = Vec::with_capacity(ZONE1_NPCS.len() + ZONE2_NPCS.len() + ZONE3_NPCS.len());
    all.extend_from_slice(ZONE1_NPCS);
    all.extend_from_slice(ZONE2_NPCS);
    all.extend_from_slice(ZONE3_NPCS);
    Box::leak(all.into_boxed_slice())
});

pub fn npc(id: &str) -> Option<&'static NpcDef> {
    NPCS.iter().find(|n| n.id == id)
}
