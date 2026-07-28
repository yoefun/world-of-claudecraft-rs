//! Graveyard spawn points for death / spirit release (absolute strip coords).

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
        x: -2.0,
        z: 8.0,
    },
    GraveyardDef {
        id: "mirefen_graveyard",
        zone_id: "mirefen",
        // Upstream Fenbridge graveyard ~(-18, 286).
        x: -18.0,
        z: 286.0,
    },
    GraveyardDef {
        id: "eastfen_graveyard",
        zone_id: "eastfen",
        x: -18.0,
        z: 286.0,
    },
    GraveyardDef {
        id: "thornpeak_graveyard",
        zone_id: "thornpeak",
        // Upstream Highwatch ~(15, 645).
        x: 15.0,
        z: 645.0,
    },
];

pub fn graveyard(id: &str) -> Option<&'static GraveyardDef> {
    GRAVEYARDS.iter().find(|g| g.id == id)
}

/// First graveyard registered for `zone_id`, if any.
pub fn graveyard_for_zone(zone_id: &str) -> Option<&'static GraveyardDef> {
    let canonical = crate::canonical_zone_id(zone_id).unwrap_or(zone_id);
    GRAVEYARDS.iter().find(|g| {
        g.zone_id == zone_id
            || crate::canonical_zone_id(g.zone_id) == Some(canonical)
            || g.zone_id == canonical
    })
}
