//! Zone 2 (Eastfen Marsh and Mirefen) NPC definitions.

use crate::npcs::{NpcDef, NpcService, VendorOffer};

pub static ZONE2_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "warden_selene",
        name: "Warden Selene",
        greeting: "The fen crawls at night. Help me keep the boardwalk clear.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastfen_circle"),
    },
    NpcDef {
        id: "apothecary_vex",
        name: "Apothecary Vex",
        greeting: "Marsh reagents, carefully labeled. Don't lick the jars.",
        services: &[
            NpcService::QuestGiver,
            NpcService::Vendor,
            NpcService::ProfessionTrainer,
        ],
        vendor_stock: &[
            VendorOffer::stack("fen_tonic", 20),
            VendorOffer::stack("travelers_ration", 20),
            VendorOffer::stack("spring_water", 40),
        ],
        trains: &["herbalism", "alchemy"],
        faction: Some("eastfen_circle"),
    },
    NpcDef {
        id: "scout_darian",
        name: "Scout Darian",
        greeting: "Fog's thick on the east channel. Warden Selene wants a word.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("eastfen_circle"),
    },
    NpcDef {
        id: "keeper_orla",
        name: "Keeper Orla",
        greeting: "Keep to the lantern posts. Mirefen swallows careless travelers.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
        faction: Some("mirefen_ferry"),
    },
    NpcDef {
        id: "ferryman_noll",
        name: "Ferryman Noll",
        greeting: "The skiff still floats, which is more than I can say for the old road.",
        services: &[NpcService::QuestGiver, NpcService::Vendor],
        vendor_stock: &[
            VendorOffer::stack("deepfen_draught", 20),
            VendorOffer::stack("travelers_ration", 20),
            VendorOffer::stack("spring_water", 40),
        ],
        trains: &[],
        faction: Some("mirefen_ferry"),
    },
];
