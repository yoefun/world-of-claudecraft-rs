use crate::professions::types::{GatherNodeDef, NodeId, NodeKind, Vec2};

pub const GATHER_NODES: &[GatherNodeDef] = &[
    GatherNodeDef {
        id: NodeId(1),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -70.0, z: -53.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(2),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -73.0, z: -49.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(3),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -67.0, z: -57.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(4),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -92.0, z: -48.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(5),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -87.0, z: -45.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(6),
        kind: NodeKind::Ore,
        pos: Vec2 { x: -65.0, z: -69.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(11),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 12.0, z: -20.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(12),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 16.0, z: -18.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(13),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 10.0, z: -24.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(14),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 40.0, z: 8.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(15),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 44.0, z: 6.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
    GatherNodeDef {
        id: NodeId(16),
        kind: NodeKind::Herb,
        pos: Vec2 { x: 38.0, z: 12.0 },
        tier: 1,
        skill_req: 0,
        respawn_seconds: 60,
    },
];

pub fn node_by_id(id: NodeId) -> Option<&'static GatherNodeDef> {
    GATHER_NODES.iter().find(|n| n.id == id)
}

/// Herb nodes 11-13 are silverleaf; 14-16 are earthroot.
pub fn herb_is_earthroot(id: NodeId) -> bool {
    id.0 >= 14
}
