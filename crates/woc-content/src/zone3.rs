//! Zone 3 — Thornpeak Heights (Highwatch hub).

use crate::zone1::{MobSpot, NpcSpot, ZoneLayout};

pub static THORNPEAK: ZoneLayout = ZoneLayout {
    name: "Thornpeak Heights",
    player_spawn_x: 2.0,
    player_spawn_z: 664.0,
    npcs: &[NpcSpot {
        npc_id: "warden_selene",
        x: 0.0,
        z: 666.0,
    }],
    mobs: &[
        MobSpot {
            mob_id: "fen_crawler",
            x: -50.0,
            z: 590.0,
        },
        MobSpot {
            mob_id: "fen_crawler",
            x: 45.0,
            z: 600.0,
        },
        MobSpot {
            mob_id: "scarred_wolf",
            x: 75.0,
            z: 625.0,
        },
        MobSpot {
            mob_id: "young_wolf",
            x: -90.0,
            z: 700.0,
        },
    ],
};
