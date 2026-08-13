//! Inventory helpers exposed on Sim.

use crate::ecs::components::Bags;
use crate::ecs::World;
use crate::entity::{count_item, grant_into, remove_item};
use woc_protocol::{EntityId, SimEvent};

pub fn grant_item(
    world: &mut World,
    player_id: EntityId,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> Result<(), &'static str> {
    let Some(bags) = world.get_mut::<Bags>(player_id) else {
        return Err("no player");
    };
    if !grant_into(&mut bags.inventory, item_id, count) {
        return Err("inventory full");
    }
    events.push(SimEvent::ItemGained {
        player: player_id,
        item_id: item_id.to_string(),
        count,
    });
    Ok(())
}

pub fn take_item(
    world: &mut World,
    player_id: EntityId,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> Result<(), &'static str> {
    let Some(bags) = world.get_mut::<Bags>(player_id) else {
        return Err("no player");
    };
    if !remove_item(&mut bags.inventory, item_id, count) {
        return Err("not enough items");
    }
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id: item_id.to_string(),
        count,
    });
    Ok(())
}

pub fn player_item_count(world: &World, player_id: EntityId, item_id: &str) -> u32 {
    world
        .get::<Bags>(player_id)
        .map(|b| count_item(&b.inventory, item_id))
        .unwrap_or(0)
}
