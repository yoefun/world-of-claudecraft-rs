//! Player corpse marker: death location until spirit release.

use crate::ecs::components::{Spirit, Transform};
use crate::ecs::World;
use crate::entity::Entity;
use woc_protocol::EntityId;

/// Record the player's body position at the moment of death.
pub fn record_corpse(player: &mut Entity) {
    player.corpse_x = Some(player.x);
    player.corpse_z = Some(player.z);
}

/// Record corpse pose from Transform into the Spirit column.
pub fn record_corpse_world(world: &mut World, id: EntityId) {
    let Some(t) = world.get::<Transform>(id).copied() else {
        return;
    };
    if let Some(s) = world.get_mut::<Spirit>(id) {
        s.corpse_x = Some(t.x);
        s.corpse_z = Some(t.z);
    }
}

/// Clear corpse bookkeeping after respawn.
pub fn clear_corpse_marker(player: &mut Entity) {
    player.corpse_x = None;
    player.corpse_z = None;
}

/// Clear Spirit corpse coords.
pub fn clear_corpse_marker_world(world: &mut World, id: EntityId) {
    if let Some(s) = world.get_mut::<Spirit>(id) {
        s.corpse_x = None;
        s.corpse_z = None;
    }
}

/// True when a corpse position has been recorded for this player.
pub fn has_corpse_marker(player: &Entity) -> bool {
    player.corpse_x.is_some() && player.corpse_z.is_some()
}

pub fn has_corpse_marker_world(world: &World, id: EntityId) -> bool {
    world
        .get::<Spirit>(id)
        .map(|s| s.corpse_x.is_some() && s.corpse_z.is_some())
        .unwrap_or(false)
}

/// Death location, if any.
pub fn corpse_position(player: &Entity) -> Option<(f32, f32)> {
    match (player.corpse_x, player.corpse_z) {
        (Some(x), Some(z)) => Some((x, z)),
        _ => None,
    }
}
