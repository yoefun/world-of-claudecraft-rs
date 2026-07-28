//! Zone 3 (Thornpeak Heights) mob templates.

use crate::mobs::{LootEntry, MobTemplate};

pub static ZONE3_MOBS: &[MobTemplate] = &[
    MobTemplate {
        id: "ridge_stalker",
        name: "Ridge Stalker",
        level: 8,
        hp: 220.0,
        xp: 135,
        copper_min: 22,
        copper_max: 42,
        attack_damage: 22.0,
        loot: &[LootEntry {
            item_id: "wolf_fang",
            chance: 0.80,
            count: 1,
        }],
    },
    MobTemplate {
        id: "cragback_boar",
        name: "Cragback Boar",
        level: 8,
        hp: 260.0,
        xp: 145,
        copper_min: 24,
        copper_max: 44,
        attack_damage: 24.0,
        loot: &[LootEntry {
            item_id: "boar_tusk",
            chance: 0.75,
            count: 1,
        }],
    },
    MobTemplate {
        id: "gale_harpy",
        name: "Gale Harpy",
        level: 9,
        hp: 210.0,
        xp: 160,
        copper_min: 28,
        copper_max: 50,
        attack_damage: 27.0,
        loot: &[],
    },
];
