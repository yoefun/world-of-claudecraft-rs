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
    pub count: u32,
    pub radius: f32,
}

pub const fn mob(mob_id: &'static str, x: f32, z: f32) -> MobSpot {
    MobSpot {
        mob_id,
        x,
        z,
        count: 1,
        radius: 1.5,
    }
}

pub const fn pack(mob_id: &'static str, x: f32, z: f32, count: u32) -> MobSpot {
    MobSpot {
        mob_id,
        x,
        z,
        count,
        radius: 2.5,
    }
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
        NpcSpot {
            npc_id: "smith_brann",
            x: 8.0,
            z: 4.0,
        },
        NpcSpot {
            npc_id: "herbalist_wren",
            x: -6.0,
            z: 6.0,
        },
        NpcSpot {
            npc_id: "innkeeper_mara",
            x: 2.0,
            z: 8.0,
        },
    ],
    mobs: &[
        // Wolf Run (upstream POI ~(-2, 70)).
        pack("young_wolf", -15.0, 55.0, 2),
        pack("young_wolf", -8.0, 62.0, 2),
        mob("young_wolf", 20.0, 70.0),
        mob("scarred_wolf", 0.0, 95.0),
        // Boar Meadow (upstream ~(65, 0)).
        mob("young_boar", 55.0, 12.0),
        mob("young_boar", 80.0, -15.0),
        mob("young_boar", 65.0, 0.0),
    ],
};
