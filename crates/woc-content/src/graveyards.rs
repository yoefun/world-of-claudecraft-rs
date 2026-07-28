//! Graveyard spawn points for death / spirit release.

#[derive(Debug, Clone, Copy)]
pub struct GraveyardDef {
    pub id: &'static str,
    pub zone_id: &'static str,
    pub x: f32,
    pub z: f32,
}

pub static GRAVEYARDS: &[GraveyardDef] = &[
    GraveyardDef {
        id: "eastbrook_graveyard",
        zone_id: "eastbrook",
        // South of the town square (player spawn is ~2,4); near the chapel path.
        x: -2.0,
        z: 8.0,
    },
    GraveyardDef {
        id: "mirefen_graveyard",
        zone_id: "mirefen",
        // Behind the lantern camp, safely east of the drowned landing.
        x: 4.0,
        z: 14.0,
    },
];

pub fn graveyard(id: &str) -> Option<&'static GraveyardDef> {
    GRAVEYARDS.iter().find(|g| g.id == id)
}

/// First graveyard registered for `zone_id`, if any.
pub fn graveyard_for_zone(zone_id: &str) -> Option<&'static GraveyardDef> {
    GRAVEYARDS.iter().find(|g| g.zone_id == zone_id)
}
