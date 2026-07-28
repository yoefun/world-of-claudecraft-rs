//! Mob templates.

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

pub static MOBS: &[MobTemplate] = &[
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
        loot: &[LootEntry {
            item_id: "wolf_fang",
            chance: 0.75,
            count: 1,
        }],
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
        loot: &[LootEntry {
            item_id: "boar_tusk",
            chance: 0.85,
            count: 1,
        }],
    },
];

pub fn mob(id: &str) -> Option<&'static MobTemplate> {
    MOBS.iter().find(|m| m.id == id)
}
