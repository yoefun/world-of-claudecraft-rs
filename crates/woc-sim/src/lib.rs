//! Deterministic World of ClaudeCraft sim (framework slice).
//!
//! Host-agnostic: no Bevy, no networking, no wall clock.

pub mod combat;
pub mod context;
pub mod corpse;
pub mod death;
pub mod entity;
pub mod host;
pub mod interaction;
pub mod inventory;
pub mod mob;
pub mod pet;
pub mod physics;
pub mod player_motion;
pub mod quests;
pub mod rng;
pub mod sim;
pub mod spirit;
pub mod stats;
pub mod targeting;
pub mod types;
pub mod world;

pub use entity::QuestState;
pub use sim::{Sim, MAX_REALM_PLAYERS};
pub use woc_content::PlayerClass;
pub use world::{terrain_height, WORLD_HALF, WORLD_SEED};
