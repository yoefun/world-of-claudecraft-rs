//! Shared tuning constants.

pub const RUN_SPEED: f32 = 7.0;
pub const PLAYER_RADIUS: f32 = 0.4;
pub const MOB_RADIUS: f32 = 0.45;
pub const MELEE_RANGE: f32 = 2.5;
pub const RANGED_FALLBACK: f32 = 18.0;
pub const AGGRO_RANGE: f32 = 18.0;
pub const LEASH_RANGE: f32 = 40.0;
pub const INTERACT_RANGE: f32 = 4.0;
pub const LOOT_RANGE: f32 = 2.0;
pub const BACKPACK_SLOTS: usize = 16;
pub const BANK_SLOTS: usize = 24;
pub const PLAYER_SWING_SEC: f32 = 2.0;
pub const MOB_SWING_SEC: f32 = 2.0;
pub const MOB_SPEED: f32 = 5.5;

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

pub fn player_hp(base: f32, level: u32) -> f32 {
    base + 18.0 * (level.saturating_sub(1) as f32)
}
