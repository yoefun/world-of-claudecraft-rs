//! Zone 3 — Thornpeak Heights (Highwatch hub).

use crate::zone1::{MobSpot, NpcSpot, ZoneLayout};

pub static THORNPEAK: ZoneLayout = ZoneLayout {
    name: "Thornpeak Heights",
    player_spawn_x: 2.0,
    player_spawn_z: 664.0,
    npcs: &[
        NpcSpot {
            npc_id: "commander_elara",
            x: 0.0,
            z: 666.0,
        },
        NpcSpot {
            npc_id: "pathfinder_toren",
            x: -6.0,
            z: 662.0,
        },
        NpcSpot {
            npc_id: "quartermaster_bren",
            x: 6.0,
            z: 661.0,
        },
    ],
    mobs: &[
        MobSpot {
            mob_id: "ridge_stalker",
            x: -70.0,
            z: 590.0,
        },
        MobSpot {
            mob_id: "ridge_stalker",
            x: -48.0,
            z: 604.0,
        },
        MobSpot {
            mob_id: "ridge_stalker",
            x: 55.0,
            z: 610.0,
        },
        MobSpot {
            mob_id: "cragback_boar",
            x: -95.0,
            z: 700.0,
        },
        MobSpot {
            mob_id: "cragback_boar",
            x: -75.0,
            z: 720.0,
        },
        MobSpot {
            mob_id: "cragback_boar",
            x: 88.0,
            z: 695.0,
        },
        MobSpot {
            mob_id: "gale_harpy",
            x: 25.0,
            z: 760.0,
        },
        MobSpot {
            mob_id: "gale_harpy",
            x: -25.0,
            z: 780.0,
        },
        MobSpot {
            mob_id: "gale_harpy",
            x: 70.0,
            z: 745.0,
        },
    ],
};
