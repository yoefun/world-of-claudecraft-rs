//! Zone 2 (Eastfen Marsh and Mirefen) mob templates.

use crate::mobs::{LootEntry, MobTemplate};

const BOG_WISP_LOOT: &[LootEntry] = &[
    LootEntry {
        item_id: "wisp_ember",
        chance: 0.65,
        count: 1,
    },
    LootEntry {
        item_id: "fen_staff",
        chance: 0.15,
        count: 1,
    },
];

const BARROW_HAG_LOOT: &[LootEntry] = &[
    LootEntry {
        item_id: "hag_claw",
        chance: 1.0,
        count: 1,
    },
    LootEntry {
        item_id: "hag_focus",
        chance: 1.0,
        count: 1,
    },
];

pub static ZONE2_MOBS: &[MobTemplate] = &[
    MobTemplate {
        id: "fen_crawler",
        name: "Fen Crawler",
        level: 3,
        hp: 90.0,
        xp: 55,
        copper_min: 8,
        copper_max: 18,
        attack_damage: 11.0,
        loot: &[LootEntry {
            item_id: "fen_silk",
            chance: 0.70,
            count: 1,
        }],
        respawn_seconds: 30.0,
        ability_id: None,
    },
    MobTemplate {
        id: "mire_toad",
        name: "Mire Toad",
        level: 3,
        hp: 100.0,
        xp: 58,
        copper_min: 9,
        copper_max: 20,
        attack_damage: 10.0,
        loot: &[LootEntry {
            item_id: "toad_bile",
            chance: 0.80,
            count: 1,
        }],
        respawn_seconds: 30.0,
        ability_id: None,
    },
    MobTemplate {
        id: "bog_wisp",
        name: "Bog Wisp",
        level: 4,
        hp: 75.0,
        xp: 70,
        copper_min: 12,
        copper_max: 24,
        attack_damage: 14.0,
        loot: BOG_WISP_LOOT,
        respawn_seconds: 30.0,
        ability_id: None,
    },
    MobTemplate {
        id: "mire_leech",
        name: "Mire Leech",
        level: 5,
        hp: 135.0,
        xp: 85,
        copper_min: 14,
        copper_max: 28,
        attack_damage: 16.0,
        loot: &[LootEntry {
            item_id: "leech_ichor",
            chance: 0.75,
            count: 1,
        }],
        respawn_seconds: 30.0,
        ability_id: None,
    },
    MobTemplate {
        id: "rotcap_shambler",
        name: "Rotcap Shambler",
        level: 6,
        hp: 165.0,
        xp: 105,
        copper_min: 18,
        copper_max: 34,
        attack_damage: 18.0,
        loot: &[LootEntry {
            item_id: "rotcap_spore",
            chance: 0.70,
            count: 1,
        }],
        respawn_seconds: 30.0,
        ability_id: None,
    },
    MobTemplate {
        id: "mire_terror",
        name: "Mire Terror",
        level: 7,
        hp: 650.0,
        xp: 450,
        copper_min: 80,
        copper_max: 120,
        attack_damage: 28.0,
        loot: &[LootEntry {
            item_id: "terror_scale",
            chance: 1.0,
            count: 1,
        }],
        respawn_seconds: 300.0,
        ability_id: Some("terror_slam"),
    },
    MobTemplate {
        id: "barrow_hag",
        name: "Barrow Hag",
        level: 6,
        hp: 320.0,
        xp: 160,
        copper_min: 24,
        copper_max: 40,
        attack_damage: 18.0,
        loot: BARROW_HAG_LOOT,
        respawn_seconds: 30.0,
        ability_id: None,
    },
];
