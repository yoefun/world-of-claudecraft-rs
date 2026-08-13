//! Deterministic World of ClaudeCraft sim.
//!
//! Host-agnostic: no Bevy, no networking, no wall clock.
//!
//! # The fat `Entity` is gone
//!
//! Actor state lives in the typed sparse columns of [`ecs::World`], never in a
//! single blob struct. Reintroducing `woc_sim::entity::Entity` breaks the
//! following doctest, which passes only while that path does not resolve:
//!
//! ```compile_fail
//! let _ = std::mem::size_of::<woc_sim::entity::Entity>();
//! ```
//!
//! A `compile_fail` doctest passes on *any* compile error, so this companion
//! proves the crate path resolves and the harness is really compiling code —
//! without it, a renamed crate would make the guard above pass vacuously.
//!
//! ```
//! let _ = std::mem::size_of::<woc_sim::ecs::components::Identity>();
//! ```

pub mod bank;
pub mod combat;
pub mod context;
pub mod corpse;
pub mod death;
pub mod delves;
pub mod ecs;
pub mod entity_motion;
pub mod host;
pub mod instances;
pub mod interaction;
pub mod inventory;
pub mod locomotion;
pub mod mail;
pub mod map_view;
pub mod market;
pub mod mob;
pub mod mount;
pub mod persist_state;
pub mod pet;
pub mod physics;
pub mod player_motion;
pub mod professions;
pub mod pvp;
pub mod quests;
pub mod reputation;
pub mod rng;
pub mod sim;
pub mod social;
pub mod spirit;
pub mod stats;
pub mod talents;
pub mod targeting;
pub mod types;
pub mod visual_catalog;
pub mod world;
pub mod worldboss;
pub mod zones;

pub use ecs::components::QuestState;
pub use locomotion::{
    desired_walk_pose, locomotion_time_scale, update_locomotion, LocoState, LocoTrack, WalkPose,
    GAIT_RUN_ENTER, GAIT_RUN_EXIT, MOVE_ENTER_SPEED, MOVE_HOLD_TIME,
};
pub use map_view::{
    paint_map_frame, paint_player_arrow, paint_terrain_rgba, pixel_to_world, region_for_zone,
    static_markers_for_region, world_to_pixel, MapMarker, MapMarkerKind, MapRegion,
};
pub use persist_state::{
    apply_player_state, create_player_from_state, export_player_state, PlayerPersistentState,
};
pub use physics::{eastbrook_buildings, Aabb};
pub use rng::{fbm2, hash2, noise2, Rng};
pub use sim::{Sim, MAX_REALM_PLAYERS, SNAPSHOT_AOI_RADIUS};
pub use visual_catalog::{
    mount_visual_spec, scene_markers, visual_key, visual_spec, zone_atmosphere, PartRole,
    PartShape, SceneMarker, SceneMarkerKind, VisualFamily, VisualPart, VisualSpec, ZoneAtmosphere,
};
pub use woc_content::PlayerClass;
pub use world::{
    ground_height, terrain_height, terrain_steepness, water_bodies, water_level, water_level_at,
    PLAYER_MAX_CLIMB_SLOPE, WORLD_HALF, WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
};
