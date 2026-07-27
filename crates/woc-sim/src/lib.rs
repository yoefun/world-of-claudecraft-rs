//! Deterministic World of ClaudeCraft sim (combat-slice).
//!
//! Host-agnostic: no Bevy, no networking, no wall clock.

pub mod combat;
pub mod entity;
pub mod mob;
pub mod player_motion;
pub mod rng;
pub mod sim;
pub mod types;
pub mod world;

pub use sim::Sim;
pub use world::{terrain_height, WORLD_HALF, WORLD_SEED};
