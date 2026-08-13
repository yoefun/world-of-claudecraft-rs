//! Player corpse marker helpers (spirit / death).

use crate::ecs::components::{Spirit, Transform};
use crate::ecs::World;
use woc_protocol::EntityId;

pub fn record_corpse_world(world: &mut World, id: EntityId) {
    let Some(t) = world.get::<Transform>(id).copied() else {
        return;
    };
    if let Some(s) = world.get_mut::<Spirit>(id) {
        s.corpse_x = Some(t.x);
        s.corpse_z = Some(t.z);
    }
}

pub fn clear_corpse_marker_world(world: &mut World, id: EntityId) {
    if let Some(s) = world.get_mut::<Spirit>(id) {
        s.corpse_x = None;
        s.corpse_z = None;
    }
}

pub fn has_corpse_marker_world(world: &World, id: EntityId) -> bool {
    world
        .get::<Spirit>(id)
        .map(|s| s.corpse_x.is_some() && s.corpse_z.is_some())
        .unwrap_or(false)
}

pub fn corpse_position_world(world: &World, id: EntityId) -> Option<(f32, f32)> {
    let s = world.get::<Spirit>(id)?;
    Some((s.corpse_x?, s.corpse_z?))
}
