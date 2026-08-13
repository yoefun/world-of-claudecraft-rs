//! Sparse component catalog. Add a column here — never a field on a blob Entity.

use crate::ecs::{SparseSet, World};
use woc_protocol::{EntityId, EntityKind};

pub trait Component: Sized + 'static {
    fn storage(world: &World) -> &SparseSet<Self>;
    fn storage_mut(world: &mut World) -> &mut SparseSet<Self>;
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub kind: EntityKind,
    pub name: String,
    pub template_id: Option<String>,
    pub zone_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

impl Component for Identity {
    fn storage(world: &World) -> &SparseSet<Self> {
        &world.identity
    }
    fn storage_mut(world: &mut World) -> &mut SparseSet<Self> {
        &mut world.identity
    }
}

impl Component for Transform {
    fn storage(world: &World) -> &SparseSet<Self> {
        &world.transform
    }
    fn storage_mut(world: &mut World) -> &mut SparseSet<Self> {
        &mut world.transform
    }
}

/// 2D ground distance using Transform columns (replaces combat::dist2d on Entity).
pub fn dist2d(world: &World, a: EntityId, b: EntityId) -> Option<f32> {
    let ta = world.get::<Transform>(a)?;
    let tb = world.get::<Transform>(b)?;
    let dx = ta.x - tb.x;
    let dz = ta.z - tb.z;
    Some((dx * dx + dz * dz).sqrt())
}
