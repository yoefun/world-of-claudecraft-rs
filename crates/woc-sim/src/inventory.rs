//! Inventory helpers exposed on Sim.

use crate::entity::{count_item, grant_into, remove_item, Entity};
use woc_protocol::SimEvent;

pub fn grant_item(
    player: &mut Entity,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> Result<(), &'static str> {
    if !grant_into(&mut player.inventory, item_id, count) {
        return Err("inventory full");
    }
    events.push(SimEvent::ItemGained {
        player: player.id,
        item_id: item_id.to_string(),
        count,
    });
    Ok(())
}

pub fn take_item(
    player: &mut Entity,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> Result<(), &'static str> {
    if !remove_item(&mut player.inventory, item_id, count) {
        return Err("not enough items");
    }
    events.push(SimEvent::ItemLost {
        player: player.id,
        item_id: item_id.to_string(),
        count,
    });
    Ok(())
}

pub fn player_item_count(player: &Entity, item_id: &str) -> u32 {
    count_item(&player.inventory, item_id)
}
