//! Shared tuning constants for the combat slice.

pub const RUN_SPEED: f32 = 7.0;
pub const PLAYER_RADIUS: f32 = 0.4;
pub const MOB_RADIUS: f32 = 0.45;
pub const MELEE_RANGE: f32 = 2.5;
pub const AGGRO_RANGE: f32 = 18.0;
pub const LEASH_RANGE: f32 = 40.0;

pub const WARRIOR_HP_BASE: f32 = 60.0;
pub const WARRIOR_HP_PER_LEVEL: f32 = 20.0;
pub const WARRIOR_RAGE_MAX: f32 = 100.0;
pub const WARRIOR_WEAPON_DAMAGE: f32 = 8.0;
pub const WARRIOR_SWING_SEC: f32 = 2.0;
pub const HEROIC_STRIKE_COST: f32 = 15.0;
pub const HEROIC_STRIKE_BONUS: f32 = 12.0;
pub const HEROIC_STRIKE_CD: f32 = 3.0;

pub const WOLF_HP: f32 = 45.0;
pub const WOLF_DAMAGE: f32 = 4.0;
pub const WOLF_SWING_SEC: f32 = 2.0;
pub const WOLF_SPEED: f32 = 5.5;
pub const WOLF_XP: u32 = 40;
pub const WOLF_COPPER_MIN: u32 = 4;
pub const WOLF_COPPER_MAX: u32 = 12;

/// Classic-inspired tiny XP table for levels 1..10.
pub fn xp_to_next(level: u32) -> u32 {
    match level {
        1 => 100,
        2 => 200,
        3 => 350,
        4 => 550,
        5 => 800,
        6 => 1100,
        7 => 1450,
        8 => 1850,
        9 => 2300,
        _ => 2800,
    }
}

pub fn warrior_hp(level: u32) -> f32 {
    WARRIOR_HP_BASE + WARRIOR_HP_PER_LEVEL * (level.saturating_sub(1) as f32)
}
