//! NPC definitions.

use std::sync::LazyLock;

use crate::npcs_zone2::ZONE2_NPCS;
use crate::npcs_zone3::ZONE3_NPCS;

#[derive(Debug, Clone)]
pub struct VendorOffer {
    pub item_id: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcService {
    QuestGiver,
    Vendor,
    Repair,
    ProfessionTrainer,
    ClassTrainer,
    Innkeeper,
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub id: &'static str,
    pub name: &'static str,
    pub greeting: &'static str,
    pub services: &'static [NpcService],
    pub vendor_stock: &'static [VendorOffer],
    pub trains: &'static [&'static str],
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
    },
    NpcDef {
        id: "trader_wilkes",
        name: "Trader Wilkes",
        greeting: "Fresh rations and a fair price, traveler.",
        services: &[NpcService::QuestGiver, NpcService::Vendor],
        vendor_stock: &[
            VendorOffer {
                item_id: "travelers_ration",
                count: 20,
            },
            VendorOffer {
                item_id: "baked_bread",
                count: 40,
            },
            VendorOffer {
                item_id: "spring_water",
                count: 40,
            },
        ],
        trains: &[],
    },
    NpcDef {
        id: "town_crier",
        name: "Town Crier",
        greeting: "Hear ye! Eastbrook stands, and the Vale endures.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
    },
    NpcDef {
        id: "eastbrook_courier",
        name: "Eastbrook Courier",
        greeting: "Stay close — the north road is not kind to messengers.",
        services: &[],
        vendor_stock: &[],
        trains: &[],
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
            VendorOffer {
                item_id: "worn_sword",
                count: 1,
            },
            VendorOffer {
                item_id: "wooden_buckler",
                count: 1,
            },
            VendorOffer {
                item_id: "copper_shortsword",
                count: 1,
            },
            VendorOffer {
                item_id: "recruit_tunic",
                count: 1,
            },
            VendorOffer {
                item_id: "coarse_whetstone",
                count: 20,
            },
            VendorOffer {
                item_id: "minor_wizard_oil",
                count: 20,
            },
        ],
        trains: &["mining", "blacksmithing"],
    },
    NpcDef {
        id: "herbalist_wren",
        name: "Herbalist Wren",
        greeting: "The vale still grows, if you know where to kneel.",
        services: &[NpcService::ProfessionTrainer],
        vendor_stock: &[],
        trains: &["herbalism", "alchemy"],
    },
    NpcDef {
        id: "innkeeper_mara",
        name: "Innkeeper Mara",
        greeting: "Rest the night. I'll keep the hearth.",
        services: &[NpcService::Innkeeper],
        vendor_stock: &[],
        trains: &[],
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
