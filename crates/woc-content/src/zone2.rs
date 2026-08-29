//! Zone 2 / 3 spawn layouts — absolute strip coordinates (Fenbridge / Highwatch).

use crate::zone1::{mob, NpcSpot, ZoneLayout};

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
        mob("fen_crawler", -40.0, 230.0),
        mob("fen_crawler", -35.0, 235.0),
        mob("fen_crawler", 35.0, 225.0),
        mob("fen_crawler", 30.0, 228.0),
        mob("mire_toad", -82.0, 273.0),
        mob("mire_toad", -120.0, 350.0),
        mob("mire_toad", -110.0, 310.0),
        mob("bog_wisp", 70.0, 300.0),
        mob("bog_wisp", 95.0, 340.0),
        mob("bog_wisp", 80.0, 315.0),
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
        mob("mire_toad", 90.0, 420.0),
        mob("mire_toad", 115.0, 450.0),
        mob("fen_crawler", -80.0, 420.0),
        mob("fen_crawler", -105.0, 455.0),
        mob("bog_wisp", 15.0, 470.0),
        mob("mire_terror", 149.5, 295.0),
    ],
};
