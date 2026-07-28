//! Player corpse marker: death location until spirit release.

use crate::entity::Entity;

/// Record the player's body position at the moment of death.
pub fn record_corpse(player: &mut Entity) {
    player.corpse_x = Some(player.x);
    player.corpse_z = Some(player.z);
}

/// Clear corpse bookkeeping after respawn.
pub fn clear_corpse_marker(player: &mut Entity) {
    player.corpse_x = None;
    player.corpse_z = None;
}

/// True when a corpse position has been recorded for this player.
pub fn has_corpse_marker(player: &Entity) -> bool {
    player.corpse_x.is_some() && player.corpse_z.is_some()
}

/// Death location, if any.
pub fn corpse_position(player: &Entity) -> Option<(f32, f32)> {
    match (player.corpse_x, player.corpse_z) {
        (Some(x), Some(z)) => Some((x, z)),
        _ => None,
    }
}
