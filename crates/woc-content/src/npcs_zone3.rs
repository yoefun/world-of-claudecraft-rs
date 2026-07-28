//! Zone 3 (Thornpeak Heights) NPC definitions.

use crate::npcs::{NpcDef, VendorOffer};

pub static ZONE3_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "commander_elara",
        name: "Commander Elara",
        greeting: "Highwatch holds the pass, but every gale brings claws to our walls.",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "pathfinder_toren",
        name: "Pathfinder Toren",
        greeting: "Watch the shale underfoot. The ridge beasts hear a stumble from a mile away.",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "quartermaster_bren",
        name: "Quartermaster Bren",
        greeting: "Cold climbs fast up here. Stock your pack before you leave the watchfires.",
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
];
