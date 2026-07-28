//! Eastbrook Vale spawn layout — absolute strip coordinates (zone1).

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
        // Wolf Run (upstream POI ~(-2, 70)).
        MobSpot {
            mob_id: "young_wolf",
            x: -15.0,
            z: 55.0,
        },
        MobSpot {
            mob_id: "young_wolf",
            x: -8.0,
            z: 62.0,
        },
        MobSpot {
            mob_id: "young_wolf",
            x: 20.0,
            z: 70.0,
        },
        MobSpot {
            mob_id: "scarred_wolf",
            x: 0.0,
            z: 95.0,
        },
        // Boar Meadow (upstream ~(65, 0)).
        MobSpot {
            mob_id: "young_boar",
            x: 55.0,
            z: 12.0,
        },
        MobSpot {
            mob_id: "young_boar",
            x: 80.0,
            z: -15.0,
        },
        MobSpot {
            mob_id: "young_boar",
            x: 65.0,
            z: 0.0,
        },
    ],
};
