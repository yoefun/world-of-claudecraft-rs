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
            VendorOffer {
                item_id: "fen_tonic",
                count: 20,
            },
            VendorOffer {
                item_id: "travelers_ration",
                count: 20,
            },
            VendorOffer {
                item_id: "spring_water",
                count: 40,
            },
        ],
        trains: &["herbalism", "alchemy"],
    },
    NpcDef {
        id: "scout_darian",
        name: "Scout Darian",
        greeting: "Fog's thick on the east channel. Warden Selene wants a word.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
    },
    NpcDef {
        id: "keeper_orla",
        name: "Keeper Orla",
        greeting: "Keep to the lantern posts. Mirefen swallows careless travelers.",
        services: &[NpcService::QuestGiver],
        vendor_stock: &[],
        trains: &[],
    },
    NpcDef {
        id: "ferryman_noll",
        name: "Ferryman Noll",
        greeting: "The skiff still floats, which is more than I can say for the old road.",
        services: &[NpcService::QuestGiver, NpcService::Vendor],
        vendor_stock: &[
            VendorOffer {
                item_id: "deepfen_draught",
                count: 20,
            },
            VendorOffer {
                item_id: "travelers_ration",
                count: 20,
            },
            VendorOffer {
                item_id: "spring_water",
                count: 40,
            },
        ],
        trains: &[],
    },
];
