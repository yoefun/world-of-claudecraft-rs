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
        *empty = Some(InvStack::new(item_id, add));
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

pub fn take_from_slot(
    inv: &mut [Option<InvStack>],
    slot: usize,
    count: u32,
) -> Option<InvStack> {
    let stack = inv.get_mut(slot)?.as_mut()?;
    let take = count.min(stack.count).max(1);
    if take < stack.count {
        stack.count -= take;
        let mut taken = stack.clone();
        taken.count = take;
        Some(taken)
    } else {
        inv[slot].take()
    }
}

fn max_stack_for(item_id: &str) -> u32 {
    let stack_size = woc_content::item(item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    if unstacked {
        1
    } else {
        stack_size
    }
}

fn stacks_merge(a: &InvStack, b: &InvStack) -> bool {
    a.item_id == b.item_id && a.durability == b.durability && a.enchant_id == b.enchant_id
}

pub fn put_stack(inv: &mut [Option<InvStack>], mut stack: InvStack) -> Result<(), InvStack> {
    if stack.count == 0 {
        return Ok(());
    }
    let max_stack = max_stack_for(&stack.item_id);
    if max_stack > 1 {
        for slot in inv.iter_mut().flatten() {
            if stacks_merge(slot, &stack) && slot.count < max_stack {
                let space = max_stack - slot.count;
                let add = stack.count.min(space);
                slot.count += add;
                stack.count -= add;
                if stack.count == 0 {
                    return Ok(());
                }
            }
        }
    }
    while stack.count > 0 {
        let Some(empty) = inv.iter_mut().find(|s| s.is_none()) else {
            return Err(stack);
        };
        let add = stack.count.min(max_stack);
        let mut placed = stack.clone();
        placed.count = add;
        stack.count -= add;
        *empty = Some(placed);
    }
    Ok(())
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

    #[test]
    fn take_from_slot_keeps_wear_and_enchant() {
        let mut inv = vec![None; 4];
        inv[1] = Some(InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(12),
            enchant_id: Some("coarse_sharpening".into()),
        });
        let taken = take_from_slot(&mut inv, 1, 1).unwrap();
        assert_eq!(taken.durability, Some(12));
        assert_eq!(taken.enchant_id.as_deref(), Some("coarse_sharpening"));
        assert!(inv[1].is_none());
    }

    #[test]
    fn take_from_slot_splits_stackable() {
        let mut inv = vec![None; 4];
        inv[0] = Some(InvStack::new("silverleaf", 5));
        let taken = take_from_slot(&mut inv, 0, 2).unwrap();
        assert_eq!(taken.count, 2);
        assert_eq!(inv[0].as_ref().unwrap().count, 3);
    }

    #[test]
    fn put_stack_merges_matching_and_rejects_full() {
        let mut inv = vec![None; 1];
        assert!(put_stack(&mut inv, InvStack::new("silverleaf", 5)).is_ok());
        assert!(put_stack(&mut inv, InvStack::new("silverleaf", 3)).is_ok());
        assert_eq!(inv[0].as_ref().unwrap().count, 8);
        let err = put_stack(&mut inv, InvStack::new("wolf_fang", 1)).unwrap_err();
        assert_eq!(err.item_id, "wolf_fang");
    }

    #[test]
    fn put_stack_does_not_merge_mismatched_enchant() {
        let mut inv = vec![None; 2];
        let a = InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(12),
            enchant_id: Some("coarse_sharpening".into()),
        };
        let b = InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(12),
            enchant_id: None,
        };
        assert!(put_stack(&mut inv, a).is_ok());
        assert!(put_stack(&mut inv, b).is_ok());
        assert!(inv[0].is_some() && inv[1].is_some());
    }
}
