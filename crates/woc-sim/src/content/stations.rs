use crate::professions::types::{StationType, Vec2};

pub struct StationDef {
    pub kind: StationType,
    pub pos: Vec2,
}

pub const STATIONS: &[StationDef] = &[
    StationDef {
        kind: StationType::Forge,
        pos: Vec2 { x: 0.0, z: 0.0 },
    },
    StationDef {
        kind: StationType::Tannery,
        pos: Vec2 { x: 80.0, z: 40.0 },
    },
    StationDef {
        kind: StationType::Loom,
        pos: Vec2 { x: 20.0, z: -10.0 },
    },
    StationDef {
        kind: StationType::JewelersBench,
        pos: Vec2 { x: 15.0, z: 5.0 },
    },
    StationDef {
        kind: StationType::Apothecary,
        pos: Vec2 { x: 7.0, z: 660.0 },
    },
    StationDef {
        kind: StationType::Toolworks,
        pos: Vec2 { x: 30.0, z: 10.0 },
    },
];
