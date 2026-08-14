//! Zone 3 — Thornpeak Heights (Highwatch hub).

use crate::zone1::{mob, NpcSpot, ZoneLayout};

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
        mob("ridge_stalker", -70.0, 590.0),
        mob("ridge_stalker", -48.0, 604.0),
        mob("ridge_stalker", 55.0, 610.0),
        mob("cragback_boar", -95.0, 700.0),
        mob("cragback_boar", -75.0, 720.0),
        mob("cragback_boar", 88.0, 695.0),
        mob("gale_harpy", 25.0, 760.0),
        mob("gale_harpy", -25.0, 780.0),
        mob("gale_harpy", 70.0, 745.0),
    ],
};
