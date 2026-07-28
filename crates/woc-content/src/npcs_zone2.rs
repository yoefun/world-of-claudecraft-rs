//! Zone 2 (Eastfen Marsh and Mirefen) NPC definitions.

use crate::npcs::{NpcDef, VendorOffer};

pub static ZONE2_NPCS: &[NpcDef] = &[
    NpcDef {
        id: "warden_selene",
        name: "Warden Selene",
        greeting: "The fen crawls at night. Help me keep the boardwalk clear.",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "apothecary_vex",
        name: "Apothecary Vex",
        greeting: "Marsh reagents, carefully labeled. Don't lick the jars.",
        is_quest_giver: true,
        is_vendor: true,
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
    },
    NpcDef {
        id: "scout_darian",
        name: "Scout Darian",
        greeting: "Fog's thick on the east channel. Warden Selene wants a word.",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "keeper_orla",
        name: "Keeper Orla",
        greeting: "Keep to the lantern posts. Mirefen swallows careless travelers.",
        is_quest_giver: true,
        is_vendor: false,
        vendor_stock: &[],
    },
    NpcDef {
        id: "ferryman_noll",
        name: "Ferryman Noll",
        greeting: "The skiff still floats, which is more than I can say for the old road.",
        is_quest_giver: true,
        is_vendor: true,
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
    },
];
