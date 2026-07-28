//! Dungeon definitions.

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
}

pub static DUNGEONS: &[DungeonDef] = &[DungeonDef {
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
}];

pub fn dungeon(id: &str) -> Option<&'static DungeonDef> {
    DUNGEONS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eastbrook_crypt_is_playable_at_level_one() {
        let crypt = dungeon("eastbrook_crypt").expect("eastbrook crypt");
        assert_eq!(crypt.zone_id, "eastbrook");
        assert_eq!(crypt.min_level, 1);
        assert!(!crypt.boss_id.is_empty());
        assert!(crypt.boss_hp > 0.0);
    }
}
