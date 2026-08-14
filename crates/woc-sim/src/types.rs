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
pub const LOOT_PILE_TTL_TICKS: u64 = 2_400;
pub const HEARTH_COOLDOWN_TICKS: u64 = 18_000;
pub const BACKPACK_SLOTS: usize = 16;
pub const BANK_SLOTS: usize = 24;
pub const PLAYER_SWING_SEC: f32 = 2.0;
pub const MOB_SWING_SEC: f32 = 2.0;
pub const MOB_SPEED: f32 = 5.5;
/// Rage gained per point of HP damage taken (warrior).
pub const RAGE_FROM_TAKEN: f32 = 0.05;
/// Energy gained per second (in or out of combat).
pub const ENERGY_REGEN_PER_SEC: f32 = 10.0;
/// Mana gained per second while out of combat.
pub const MANA_REGEN_OOC_PER_SEC: f32 = 8.0;
/// Mana gained per second while in combat.
pub const MANA_REGEN_COMBAT_PER_SEC: f32 = 2.0;
/// Rage lost per second while out of combat.
pub const RAGE_DECAY_OOC_PER_SEC: f32 = 3.0;
/// Stealthed horizontal speed multiplier (stacked with chill via `min`).
pub const STEALTH_MOVE_MULT: f32 = 0.7;
/// Threat required on a challenger before a mob retargets away from its current focus.
pub const THREAT_SWITCH_RATIO: f32 = 1.1;

/// White-hit table (player auto-attack and damaging abilities).
pub const MISS_CHANCE: f32 = 0.05;
pub const CRIT_CHANCE: f32 = 0.10;
pub const CRIT_MULT: f32 = 2.0;

/// Upstream-aligned vertical motion constants (player_motion.ts).
pub const GRAVITY: f32 = 16.0;
pub const JUMP_VELOCITY: f32 = 6.0;
pub const AIR_CONTROL_ACCEL: f32 = 20.0;
pub const COYOTE_TIME: f32 = 0.15;
pub const FALL_SAFE_DISTANCE: f32 = 12.0;
pub const SWIM_SPEED_MULT: f32 = 0.65;
pub const FLY_SPEED_MULT: f32 = 1.15;
pub const FLY_VERTICAL_SPEED: f32 = 5.5;
/// Ground this far under the water line = deep water (upstream PLAYER_SWIM_DEPTH).
pub const PLAYER_SWIM_DEPTH: f32 = 1.4;

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
