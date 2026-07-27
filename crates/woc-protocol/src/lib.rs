//! Shared wire / host types between sim, client, and (future) server.

use serde::{Deserialize, Serialize};

/// Stable entity identifier within one `Sim` instance.
pub type EntityId = u32;

/// Fixed sim rate matching upstream World of ClaudeCraft.
pub const TICK_RATE: u32 = 20;
pub const DT: f32 = 1.0 / TICK_RATE as f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Player,
    Mob,
    Loot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilitySlot {
    /// Heroic Strike analogue.
    Primary = 1,
}

/// Per-tick intent from a local or remote player.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerIntent {
    /// Forward/back wish in [-1, 1] (camera-relative on the client).
    pub move_x: f32,
    /// Strafe wish in [-1, 1].
    pub move_z: f32,
    /// Desired yaw in radians (world space).
    pub facing: f32,
    /// Start/continue auto-attack against `target_id`.
    pub attack: bool,
    /// Fire ability on this slot (if ready).
    pub ability: Option<AbilitySlot>,
    /// Selected target (mob or none).
    pub target_id: Option<EntityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub kind: EntityKind,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub hp: f32,
    pub hp_max: f32,
    pub level: u32,
    pub name: String,
    /// Rage for warriors; unused for mobs/loot.
    pub resource: f32,
    pub resource_max: f32,
    pub alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProgress {
    pub xp: u32,
    pub xp_to_level: u32,
    pub level: u32,
    pub copper: u32,
    pub bag_item: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSnapshot {
    pub tick: u64,
    pub player_id: EntityId,
    pub entities: Vec<EntitySnapshot>,
    pub progress: PlayerProgress,
    pub target_id: Option<EntityId>,
    pub ability_ready: bool,
    pub ability_cooldown: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SimEvent {
    Damage {
        source: EntityId,
        target: EntityId,
        amount: f32,
        ability: Option<String>,
    },
    Kill {
        killer: EntityId,
        victim: EntityId,
        victim_name: String,
    },
    Loot {
        player: EntityId,
        copper: u32,
        item: Option<String>,
    },
    LevelUp {
        player: EntityId,
        level: u32,
    },
    Toast {
        message: String,
    },
}
