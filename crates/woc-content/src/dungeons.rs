//! Dungeon definitions.

#[derive(Debug, Clone, Copy)]
pub struct DungeonTrashSpot {
    pub mob_id: &'static str,
    pub x: f32,
    pub z: f32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct DungeonDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Overworld zone containing the entrance and return spawn.
    pub zone_id: &'static str,
    pub min_level: u32,
    pub boss_id: &'static str,
    pub boss_name: &'static str,
    pub boss_level: u32,
    pub boss_hp: f32,
    pub boss_attack_damage: f32,
    pub entrance_x: f32,
    pub entrance_z: f32,
    pub boss_x: f32,
    pub boss_z: f32,
    pub trash: &'static [DungeonTrashSpot],
}

pub static DUNGEONS: &[DungeonDef] = &[
    DungeonDef {
        id: "eastbrook_crypt",
        name: "Eastbrook Crypt",
        zone_id: "eastbrook",
        min_level: 1,
        boss_id: "crypt_warden",
        boss_name: "The Crypt Warden",
        boss_level: 3,
        boss_hp: 240.0,
        boss_attack_damage: 14.0,
        entrance_x: -8.0,
        entrance_z: 0.0,
        boss_x: 12.0,
        boss_z: 0.0,
        trash: &[
            DungeonTrashSpot {
                mob_id: "young_wolf",
                x: 0.0,
                z: 2.0,
                count: 2,
            },
            DungeonTrashSpot {
                mob_id: "young_boar",
                x: 4.0,
                z: -2.0,
                count: 2,
            },
        ],
    },
    DungeonDef {
        id: "mirefen_barrow",
        name: "Mirefen Barrow",
        zone_id: "mirefen",
        min_level: 3,
        boss_id: "barrow_hag",
        boss_name: "The Barrow Hag",
        boss_level: 6,
        boss_hp: 320.0,
        boss_attack_damage: 18.0,
        entrance_x: 25.0,
        entrance_z: 430.0,
        boss_x: 40.0,
        boss_z: 445.0,
        trash: &[
            DungeonTrashSpot {
                mob_id: "fen_crawler",
                x: 30.0,
                z: 434.0,
                count: 2,
            },
            DungeonTrashSpot {
                mob_id: "mire_toad",
                x: 34.0,
                z: 438.0,
                count: 2,
            },
        ],
    },
];

pub fn dungeon(id: &str) -> Option<&'static DungeonDef> {
    DUNGEONS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob;

    #[test]
    fn eastbrook_crypt_is_playable_at_level_one() {
        let crypt = dungeon("eastbrook_crypt").expect("eastbrook crypt");
        assert_eq!(crypt.zone_id, "eastbrook");
        assert_eq!(crypt.min_level, 1);
        assert!(!crypt.boss_id.is_empty());
        assert!(crypt.boss_hp > 0.0);
        assert!(crypt.trash.len() >= 2);
    }

    #[test]
    fn mirefen_barrow_is_a_second_instance() {
        let barrow = dungeon("mirefen_barrow").expect("mirefen barrow");
        assert_eq!(barrow.zone_id, "mirefen");
        assert!(barrow.min_level >= 1);
        assert_eq!(barrow.boss_id, "barrow_hag");
        assert!(barrow.boss_hp > 0.0);
        assert!(barrow.trash.len() >= 2);
        assert!(mob(barrow.boss_id).is_some());
    }

    #[test]
    fn dungeon_trash_mobs_resolve() {
        for def in DUNGEONS {
            for spot in def.trash {
                assert!(
                    mob(spot.mob_id).is_some(),
                    "dungeon {} trash refs missing mob {}",
                    def.id,
                    spot.mob_id
                );
                assert!(spot.count >= 1);
            }
        }
    }
}
