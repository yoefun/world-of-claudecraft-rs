//! Deterministic World of ClaudeCraft sim (framework slice).
//!
//! Host-agnostic: no Bevy, no networking, no wall clock.

pub mod bank;
pub mod combat;
pub mod context;
pub mod corpse;
pub mod death;
pub mod delves;
pub mod entity;
pub mod host;
pub mod instances;
pub mod interaction;
pub mod inventory;
pub mod mail;
pub mod map_view;
pub mod market;
pub mod mob;
pub mod persist_state;
pub mod pet;
pub mod physics;
pub mod player_motion;
pub mod professions;
pub mod pvp;
pub mod quests;
pub mod rng;
pub mod sim;
pub mod social;
pub mod spirit;
pub mod stats;
pub mod talents;
pub mod targeting;
pub mod types;
pub mod world;
pub mod worldboss;
pub mod zones;

pub use entity::QuestState;
pub use map_view::{
    paint_map_frame, paint_player_arrow, paint_terrain_rgba, pixel_to_world, region_for_zone,
    static_markers_for_region, world_to_pixel, MapMarker, MapMarkerKind, MapRegion,
};
pub use persist_state::{
    apply_player_state, create_player_from_state, export_player_state, PlayerPersistentState,
};
pub use rng::{fbm2, hash2, noise2, Rng};
pub use sim::{Sim, MAX_REALM_PLAYERS};
pub use woc_content::PlayerClass;
pub use world::{
    ground_height, terrain_height, terrain_steepness, water_bodies, water_level, water_level_at,
    PLAYER_MAX_CLIMB_SLOPE, WORLD_HALF, WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
};
