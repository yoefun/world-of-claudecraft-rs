//! Mob templates.

use std::sync::LazyLock;

use crate::mobs_zone2::ZONE2_MOBS;
use crate::mobs_zone3::ZONE3_MOBS;

#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_id: &'static str,
    /// Chance in [0, 1].
    pub chance: f32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct MobTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub level: u32,
    pub hp: f32,
    pub xp: u32,
    pub copper_min: u32,
    pub copper_max: u32,
    pub attack_damage: f32,
    pub loot: &'static [LootEntry],
}

const SCARRED_WOLF_LOOT: &[LootEntry] = &[
    LootEntry {
        item_id: "wolf_fang",
        chance: 0.75,
        count: 1,
    },
    LootEntry {
        item_id: "fang_pendant",
        chance: 0.12,
        count: 1,
    },
];

const YOUNG_BOAR_LOOT: &[LootEntry] = &[
    LootEntry {
        item_id: "boar_tusk",
        chance: 0.85,
        count: 1,
    },
    LootEntry {
        item_id: "boar_tusk_ring",
        chance: 0.12,
        count: 1,
    },
];

const CRYPT_WARDEN_LOOT: &[LootEntry] = &[
    LootEntry {
        item_id: "crypt_cleaver",
        chance: 1.0,
        count: 1,
    },
];

pub static ZONE1_MOBS: &[MobTemplate] = &[
    MobTemplate {
        id: "young_wolf",
        name: "Young Wolf",
        level: 1,
        hp: 45.0,
        xp: 25,
        copper_min: 3,
        copper_max: 8,
        attack_damage: 6.0,
        loot: &[LootEntry {
            item_id: "wolf_fang",
            chance: 0.55,
            count: 1,
        }],
    },
    MobTemplate {
        id: "scarred_wolf",
        name: "Scarred Wolf",
        level: 2,
        hp: 70.0,
        xp: 40,
        copper_min: 6,
        copper_max: 14,
        attack_damage: 9.0,
        loot: SCARRED_WOLF_LOOT,
    },
    MobTemplate {
        id: "young_boar",
        name: "Young Boar",
        level: 1,
        hp: 55.0,
        xp: 28,
        copper_min: 4,
        copper_max: 10,
        attack_damage: 7.0,
        loot: YOUNG_BOAR_LOOT,
    },
    MobTemplate {
        id: "crypt_warden",
        name: "The Crypt Warden",
        level: 3,
        hp: 240.0,
        xp: 150,
        copper_min: 20,
        copper_max: 40,
        attack_damage: 14.0,
        loot: CRYPT_WARDEN_LOOT,
    },
];

/// Zone1 + zone2 + zone3 mob templates.
pub static MOBS: LazyLock<&'static [MobTemplate]> = LazyLock::new(|| {
    let mut all = Vec::with_capacity(ZONE1_MOBS.len() + ZONE2_MOBS.len() + ZONE3_MOBS.len());
    all.extend_from_slice(ZONE1_MOBS);
    all.extend_from_slice(ZONE2_MOBS);
    all.extend_from_slice(ZONE3_MOBS);
    Box::leak(all.into_boxed_slice())
});

pub fn mob(id: &str) -> Option<&'static MobTemplate> {
    MOBS.iter().find(|m| m.id == id)
}
