//! Inventory helpers exposed on Sim and used by economy / interaction.

use woc_content::ItemKind;
use woc_protocol::{EntityId, SimEvent};

use crate::ecs::components::{Bags, InvStack};
use crate::ecs::World;

/// Remove `count` from a specific bag slot, preserving durability / enchant.
pub fn take_from_slot(inv: &mut [Option<InvStack>], slot: u8, count: u32) -> Option<InvStack> {
    let entry = inv.get_mut(slot as usize)?;
    let mut stack = entry.take()?;
    let take = count.min(stack.count).max(1);
    if take > stack.count {
        *entry = Some(stack);
        return None;
    }
    let taken = InvStack {
        item_id: stack.item_id.clone(),
        count: take,
        durability: stack.durability,
        enchant_id: stack.enchant_id.clone(),
    };
    stack.count -= take;
    if stack.count > 0 {
        *entry = Some(stack);
    }
    Some(taken)
}

/// Insert a concrete stack. Merge only with the same item_id + durability + enchant_id
/// when the catalog stack size allows. Weapons/armor stay unstacked.
pub fn grant_stack(inv: &mut [Option<InvStack>], incoming: InvStack) -> bool {
    if incoming.count == 0 {
        return true;
    }
    let stack_size = woc_content::item(&incoming.item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(&incoming.item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    let max_stack = if unstacked { 1 } else { stack_size };
    let mut remaining = incoming.count;
    if max_stack > 1 {
        for stack in inv.iter_mut().flatten() {
            if stack.item_id == incoming.item_id
                && stack.durability == incoming.durability
                && stack.enchant_id == incoming.enchant_id
                && stack.count < max_stack
            {
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
            item_id: incoming.item_id.clone(),
            count: add,
            durability: incoming.durability,
            enchant_id: incoming.enchant_id.clone(),
        });
        remaining -= add;
    }
    true
}

/// Insert into backpack with stacking. Returns false if full.
pub fn grant_into(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    grant_stack(inv, InvStack::new(item_id, count))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::InvStack;

    fn empty_bags() -> [Option<InvStack>; 4] {
        [None, None, None, None]
    }

    #[test]
    fn take_from_slot_preserves_wear_and_leaves_the_other_stack() {
        let mut inv = empty_bags();
        inv[0] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 3,
            durability: None,
            enchant_id: None,
        });
        inv[1] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 2,
            durability: None,
            enchant_id: None,
        });
        let taken = take_from_slot(&mut inv, 1, 1).unwrap();
        assert_eq!(taken.count, 1);
        assert_eq!(inv[0].as_ref().unwrap().count, 3);
        assert_eq!(inv[1].as_ref().unwrap().count, 1);
    }

    #[test]
    fn grant_stack_keeps_worn_enchanted_sword_unmerged() {
        let mut inv = empty_bags();
        let worn = InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
        };
        assert!(grant_stack(&mut inv, worn.clone()));
        assert_eq!(inv[0], Some(worn));
        assert!(grant_into(&mut inv, "worn_sword", 1));
        assert_eq!(inv[1].as_ref().unwrap().durability, Some(40));
        assert!(inv[1].as_ref().unwrap().enchant_id.is_none());
    }
}
