//! Zone 2 / 3 spawn layouts — absolute strip coordinates (Fenbridge / Highwatch).

use crate::zone1::{MobSpot, NpcSpot, ZoneLayout};

/// Eastfen Marsh alias layout — Fenbridge hub (upstream mirefen_marsh).
pub static EASTFEN: ZoneLayout = ZoneLayout {
    name: "Mirefen Marsh",
    player_spawn_x: 2.0,
    player_spawn_z: 304.0,
    npcs: &[
        NpcSpot {
            npc_id: "warden_selene",
            x: 0.0,
            z: 306.0,
        },
        NpcSpot {
            npc_id: "apothecary_vex",
            x: 5.0,
            z: 303.0,
        },
        NpcSpot {
            npc_id: "scout_darian",
            x: -3.0,
            z: 302.0,
        },
    ],
    mobs: &[
        // Prowler Reeds ~(-40, 230).
        MobSpot {
            mob_id: "fen_crawler",
            x: -40.0,
            z: 230.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -35.0,
            z: 235.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: 35.0,
            z: 225.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: 30.0,
            z: 228.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: -82.0,
            z: 273.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: -120.0,
            z: 350.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: -110.0,
            z: 310.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 70.0,
            z: 300.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 95.0,
            z: 340.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 80.0,
            z: 315.0,
        },
    ],
};

/// Mirefen deep-marsh camp — same zone band, north of Fenbridge.
pub static MIREFEN: ZoneLayout = ZoneLayout {
    name: "Mirefen",
    player_spawn_x: 3.0,
    player_spawn_z: 308.0,
    npcs: &[
        NpcSpot {
            npc_id: "keeper_orla",
            x: 1.0,
            z: 311.0,
        },
        NpcSpot {
            npc_id: "ferryman_noll",
            x: 7.0,
            z: 306.0,
        },
    ],
    mobs: &[
        MobSpot {
            mob_id: "mire_toad",
            x: 90.0,
            z: 420.0,
        },
        MobSpot {
            mob_id: "mire_toad",
            x: 115.0,
            z: 450.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -80.0,
            z: 420.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: -105.0,
            z: 455.0,
        },
        MobSpot {
            mob_id: "bog_wisp",
            x: 15.0,
            z: 470.0,
        },
        MobSpot {
            mob_id: "mire_terror",
            x: 149.5,
            z: 295.0,
        },
    ],
};
