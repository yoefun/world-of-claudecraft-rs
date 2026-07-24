//! NPC definitions.

use std::sync::LazyLock;

use crate::npcs_zone2::ZONE2_NPCS;

#[derive(Debug, Clone)]
pub struct VendorOffer {
    pub item_id: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub id: &'static str,
    pub name: &'static str,
    pub greeting: &'static str,
    pub is_quest_giver: bool,
    pub is_vendor: bool,
    pub vendor_stock: &'static [VendorOffer],
}

pub static ZONE1_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "captain_alden",
        name: "Captain Alden",
        greeting: "The north road is thick with wolves. Can you thin the pack?",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "trader_wilkes",
        name: "Trader Wilkes",
        greeting: "Fresh rations and a fair price, traveler.",
        is_quest_giver: false,
        is_vendor: true,
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
    },
    NpcDef {
        id: "town_crier",
        name: "Town Crier",
        greeting: "Hear ye! Eastbrook stands, and the Vale endures.",
        is_quest_giver: false,
        is_vendor: false,
        vendor_stock: &[],
    },
];

/// Zone1 + zone2 NPC definitions.
pub static NPCS: LazyLock<&'static [NpcDef]> = LazyLock::new(|| {
    let mut all = Vec::with_capacity(ZONE1_NPCS.len() + ZONE2_NPCS.len());
    all.extend_from_slice(ZONE1_NPCS);
    all.extend_from_slice(ZONE2_NPCS);
    Box::leak(all.into_boxed_slice())
});

pub fn npc(id: &str) -> Option<&'static NpcDef> {
    NPCS.iter().find(|n| n.id == id)
}
