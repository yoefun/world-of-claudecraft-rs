//! Eastbrook Vale spawn layout (framework scaffold).

#[derive(Debug, Clone, Copy)]
pub struct NpcSpot {
    pub npc_id: &'static str,
    pub x: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MobSpot {
    pub mob_id: &'static str,
    pub x: f32,
    pub z: f32,
}

#[derive(Debug, Clone)]
pub struct ZoneLayout {
    pub name: &'static str,
    pub player_spawn_x: f32,
    pub player_spawn_z: f32,
    pub npcs: &'static [NpcSpot],
    pub mobs: &'static [MobSpot],
}

pub static EASTBROOK: ZoneLayout = ZoneLayout {
    name: "Eastbrook Vale",
    player_spawn_x: 2.0,
    player_spawn_z: 4.0,
    npcs: &[
        NpcSpot {
            npc_id: "captain_alden",
            x: 0.0,
            z: 6.0,
        },
        NpcSpot {
            npc_id: "trader_wilkes",
            x: 6.0,
            z: 2.0,
        },
        NpcSpot {
            npc_id: "town_crier",
            x: -4.0,
            z: 3.0,
        },
    ],
    mobs: &[
        // Wolf camp north.
        MobSpot {
            mob_id: "young_wolf",
            x: -8.0,
            z: -22.0,
        },
        MobSpot {
            mob_id: "young_wolf",
            x: -4.0,
            z: -26.0,
        },
        MobSpot {
            mob_id: "young_wolf",
            x: 2.0,
            z: -24.0,
        },
        MobSpot {
            mob_id: "scarred_wolf",
            x: 6.0,
            z: -28.0,
        },
        // Boar meadow east.
        MobSpot {
            mob_id: "young_boar",
            x: 22.0,
            z: 4.0,
        },
        MobSpot {
            mob_id: "young_boar",
            x: 26.0,
            z: 8.0,
        },
        MobSpot {
            mob_id: "young_boar",
            x: 28.0,
            z: 0.0,
        },
    ],
};
