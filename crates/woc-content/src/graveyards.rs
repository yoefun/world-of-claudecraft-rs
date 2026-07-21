//! Graveyard spawn points (stub for death / spirit waves).

#[derive(Debug, Clone, Copy)]
pub struct GraveyardDef {
    pub id: &'static str,
    pub zone_id: &'static str,
    pub x: f32,
    pub z: f32,
}

pub static GRAVEYARDS: &[GraveyardDef] = &[GraveyardDef {
    id: "eastbrook_graveyard",
    zone_id: "eastbrook",
    x: 0.0,
    z: 2.0,
}];

pub fn graveyard(id: &str) -> Option<&'static GraveyardDef> {
    GRAVEYARDS.iter().find(|g| g.id == id)
}
