//! Zone 2 spawn layouts — Eastfen Marsh and Mirefen.

use crate::zone1::{MobSpot, NpcSpot, ZoneLayout};

/// Eastfen Marsh — boardwalk outpost, crawler nests, toad pools, and wisp reeds.
pub static EASTFEN: ZoneLayout = ZoneLayout {
    name: "Eastfen Marsh",
    player_spawn_x: 2.0,
    player_spawn_z: 4.0,
    npcs: &[
        NpcSpot {
            npc_id: "warden_selene",
            x: 0.0,
            z: 6.0,
        },
        NpcSpot {
            npc_id: "apothecary_vex",
            x: 5.0,
            z: 3.0,
        },
        NpcSpot {
            npc_id: "scout_darian",
            x: -3.0,
            z: 2.0,
        },
    ],
    mobs: &[
        // Fen crawler camp — west of the boardwalk.
        MobSpot {
            mob_id: "fen_crawler",
            x: -18.0,
            z: -6.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -22.0,
            z: -2.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -20.0,
            z: 4.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -16.0,
            z: 0.0,
        },
        // Mire toad pools — south.
        MobSpot {
            mob_id: "mire_toad",
            x: 4.0,
            z: -20.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: 10.0,
            z: -24.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: 0.0,
            z: -26.0,
        },
        // Bog wisp reeds — northeast.
        MobSpot {
            mob_id: "bog_wisp",
            x: 22.0,
            z: 14.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 26.0,
            z: 18.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 28.0,
            z: 10.0,
        },
    ],
};

/// Mirefen — lantern camp, drowned landing, rotcap grove, and terror sinkhole.
pub static MIREFEN: ZoneLayout = ZoneLayout {
    name: "Mirefen",
    player_spawn_x: 3.0,
    player_spawn_z: 8.0,
    npcs: &[
        NpcSpot {
            npc_id: "keeper_orla",
            x: 1.0,
            z: 11.0,
        },
        NpcSpot {
            npc_id: "ferryman_noll",
            x: 7.0,
            z: 6.0,
        },
    ],
    mobs: &[
        // Mire leeches — clustered around the drowned western landing.
        MobSpot {
            mob_id: "mire_leech",
            x: -18.0,
            z: 5.0,
        },
        MobSpot {
            mob_id: "mire_leech",
            x: -22.0,
            z: 0.0,
        },
        MobSpot {
            mob_id: "mire_leech",
            x: -16.0,
            z: -5.0,
        },
        MobSpot {
            mob_id: "mire_leech",
            x: -24.0,
            z: -8.0,
        },
        // Rotcap shamblers — beneath the southern fungus grove.
        MobSpot {
            mob_id: "rotcap_shambler",
            x: -2.0,
            z: -22.0,
        },
        MobSpot {
            mob_id: "rotcap_shambler",
            x: 5.0,
            z: -25.0,
        },
        MobSpot {
            mob_id: "rotcap_shambler",
            x: 11.0,
            z: -20.0,
        },
        // World boss — alone in the eastern sinkhole.
        MobSpot {
            mob_id: "mire_terror",
            x: 31.0,
            z: 17.0,
        },
    ],
};
