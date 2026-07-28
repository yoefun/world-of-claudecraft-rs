//! Dungeon definitions (stub for later completion waves).

#[derive(Debug, Clone)]
pub struct DungeonDef {
    pub id: &'static str,
    pub name: &'static str,
    pub zone_id: &'static str,
    pub min_level: u32,
}

pub static DUNGEONS: &[DungeonDef] = &[];

pub fn dungeon(id: &str) -> Option<&'static DungeonDef> {
    DUNGEONS.iter().find(|d| d.id == id)
}
