//! Zone 3 (Thornpeak Heights) NPC definitions.

use crate::npcs::{NpcDef, NpcService, VendorOffer};

pub static ZONE3_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "commander_elara",
        name: "Commander Elara",
        greeting: "Highwatch holds the pass, but every gale brings claws to our walls.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("highwatch"),
    },
    NpcDef {
        id: "pathfinder_toren",
        name: "Pathfinder Toren",
        greeting: "Watch the shale underfoot. The ridge beasts hear a stumble from a mile away.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("highwatch"),
    },
    NpcDef {
        id: "quartermaster_bren",
        name: "Quartermaster Bren",
        greeting: "Cold climbs fast up here. Stock your pack before you leave the watchfires.",
        services: &[NpcService::Vendor, NpcService::Repair],
        vendor_stock: &[
            VendorOffer::stack("travelers_ration", 20),
            VendorOffer::stack("baked_bread", 40),
            VendorOffer::stack("spring_water", 40),
        ],
        trains: &[],
        faction: Some("highwatch"),
    },
];
