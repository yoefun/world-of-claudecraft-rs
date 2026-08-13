//! Inventory helpers exposed on Sim and used by economy / interaction.

use woc_content::ItemKind;
use woc_protocol::{EntityId, SimEvent};

use crate::ecs::components::{Bags, InvStack};
use crate::ecs::World;

/// Insert into backpack with stacking. Returns false if full.
pub fn grant_into(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    if count == 0 {
        return true;
    }
    let stack_size = woc_content::item(item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    let max_stack = if unstacked { 1 } else { stack_size };

    let mut remaining = count;
    if max_stack > 1 {
        for stack in inv.iter_mut().flatten() {
            if stack.item_id == item_id && stack.count < max_stack {
                let space = max_stack - stack.count;
                let add = remaining.min(space);
                stack.count += add;
                remaining -= add;
                if remaining == 0 {
                    return true;
                }
            }
        }
    }
    while remaining > 0 {
        let Some(empty) = inv.iter_mut().find(|s| s.is_none()) else {
            return false;
        };
        let add = remaining.min(max_stack);
        *empty = Some(InvStack {
            item_id: item_id.to_string(),
            count: add,
        });
        remaining -= add;
    }
    true
}

pub fn count_item(inv: &[Option<InvStack>], item_id: &str) -> u32 {
    inv.iter()
        .filter_map(|s| s.as_ref())
        .filter(|s| s.item_id == item_id)
        .map(|s| s.count)
        .sum()
}

pub fn remove_item(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    if count_item(inv, item_id) < count {
        return false;
    }
    let mut remaining = count;
    for slot in inv.iter_mut() {
        if remaining == 0 {
            break;
        }
        let Some(stack) = slot.as_mut() else {
            continue;
        };
        if stack.item_id != item_id {
            continue;
        }
        let take = remaining.min(stack.count);
        stack.count -= take;
        remaining -= take;
        if stack.count == 0 {
            *slot = None;
        }
    }
    remaining == 0
}

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
