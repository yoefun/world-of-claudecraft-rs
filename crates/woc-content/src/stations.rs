//! Crafting station placements.

#[derive(Debug, Clone, Copy)]
pub struct StationDef {
    pub id: &'static str,
    pub name: &'static str,
    pub x: f32,
    pub z: f32,
}

pub const STATION_RADIUS: f32 = 20.0;

pub static STATIONS: &[StationDef] = &[
    StationDef {
        id: "forge",
        name: "Forge",
        x: 0.0,
        z: 0.0,
    },
    StationDef {
        id: "tannery",
        name: "Tannery",
        x: 80.0,
        z: 40.0,
    },
    StationDef {
        id: "loom",
        name: "Loom",
        x: 20.0,
        z: -10.0,
    },
    StationDef {
        id: "jewelers_bench",
        name: "Jeweler's Bench",
        x: 120.0,
        z: -50.0,
    },
    StationDef {
        id: "apothecary",
        name: "Apothecary",
        x: 7.0,
        z: 660.0,
    },
    StationDef {
        id: "toolworks",
        name: "Toolworks",
        x: 30.0,
        z: 10.0,
    },
];

pub fn station(id: &str) -> Option<&'static StationDef> {
    STATIONS.iter().find(|s| s.id == id)
}

pub fn in_station_range(x: f32, z: f32, station_id: &str) -> bool {
    let Some(def) = station(station_id) else {
        return false;
    };
    let dx = x - def.x;
    let dz = z - def.z;
    (dx * dx + dz * dz).sqrt() <= STATION_RADIUS
}
