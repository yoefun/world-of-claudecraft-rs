//! Riding ranks and mount definitions.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RidingRankDef {
    pub rank: u8,
    pub id: &'static str,
    pub name: &'static str,
    pub level_req: u32,
    pub copper: u32,
    pub ground_speed_mult: f32,
}

pub static RIDING_RANKS: &[RidingRankDef] = &[
    RidingRankDef {
        rank: 1,
        id: "apprentice",
        name: "Apprentice Riding",
        level_req: 2,
        copper: 10,
        ground_speed_mult: 1.6,
    },
    RidingRankDef {
        rank: 2,
        id: "journeyman",
        name: "Journeyman Riding",
        level_req: 5,
        copper: 50,
        ground_speed_mult: 2.0,
    },
    RidingRankDef {
        rank: 3,
        id: "expert",
        name: "Expert Riding",
        level_req: 8,
        copper: 200,
        ground_speed_mult: 2.0,
    },
];

pub fn riding_rank(id: &str) -> Option<&'static RidingRankDef> {
    RIDING_RANKS.iter().find(|r| r.id == id)
}

pub fn riding_rank_by_n(n: u8) -> Option<&'static RidingRankDef> {
    RIDING_RANKS.iter().find(|r| r.rank == n)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountKind {
    Ground,
    Flying,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountDef {
    pub id: &'static str,
    pub name: &'static str,
    pub item_id: &'static str,
    pub kind: MountKind,
    pub riding_rank: u8,
    pub speed_mult: f32,
    pub visual_key: &'static str,
}

pub static MOUNTS: &[MountDef] = &[
    MountDef {
        id: "brown_pony",
        name: "Brown Pony",
        item_id: "brown_pony",
        kind: MountKind::Ground,
        riding_rank: 1,
        speed_mult: 1.6,
        visual_key: "mount_pony",
    },
    MountDef {
        id: "swift_bay_steed",
        name: "Swift Bay Steed",
        item_id: "swift_bay_steed",
        kind: MountKind::Ground,
        riding_rank: 2,
        speed_mult: 2.0,
        visual_key: "mount_steed",
    },
    MountDef {
        id: "tawny_gryphon",
        name: "Tawny Gryphon",
        item_id: "tawny_gryphon",
        kind: MountKind::Flying,
        riding_rank: 3,
        speed_mult: 2.0,
        visual_key: "mount_gryphon",
    },
];

pub fn mount(id: &str) -> Option<&'static MountDef> {
    MOUNTS.iter().find(|m| m.id == id)
}

pub fn mount_by_item(item_id: &str) -> Option<&'static MountDef> {
    MOUNTS.iter().find(|m| m.item_id == item_id)
}
