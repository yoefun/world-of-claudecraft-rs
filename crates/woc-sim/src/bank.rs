//! Personal bank deposit / withdraw (items + copper vault).

use crate::entity::{grant_into, remove_item, Entity};
use crate::types::BANK_SLOTS;
use woc_protocol::{EntityId, SimEvent};

pub fn deposit(
    entities: &mut [Entity],
    player_id: EntityId,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    if player.bank.len() < BANK_SLOTS {
        player.bank.resize(BANK_SLOTS, None);
    }
    let slot = bag_slot as usize;
    let Some(Some(stack)) = player.inventory.get(slot).cloned() else {
        events.push(SimEvent::Toast {
            message: "Empty bag slot.".into(),
        });
        return false;
    };
    let take = count.min(stack.count).max(1);
    if !remove_item(&mut player.inventory, &stack.item_id, take) {
        return false;
    }
    if !grant_into(&mut player.bank, &stack.item_id, take) {
        let _ = grant_into(&mut player.inventory, &stack.item_id, take);
        events.push(SimEvent::Toast {
            message: "Bank is full.".into(),
        });
        return false;
    }
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id: stack.item_id,
        count: take,
    });
    true
}

pub fn withdraw(
    entities: &mut [Entity],
    player_id: EntityId,
    bank_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    let slot = bank_slot as usize;
    let Some(Some(stack)) = player.bank.get(slot).cloned() else {
        events.push(SimEvent::Toast {
            message: "Empty bank slot.".into(),
        });
        return false;
    };
    let take = count.min(stack.count).max(1);
    if !remove_item(&mut player.bank, &stack.item_id, take) {
        return false;
    }
    if !grant_into(&mut player.inventory, &stack.item_id, take) {
        let _ = grant_into(&mut player.bank, &stack.item_id, take);
        events.push(SimEvent::Toast {
            message: "Bags are full.".into(),
        });
        return false;
    }
    events.push(SimEvent::ItemGained {
        player: player_id,
        item_id: stack.item_id,
        count: take,
    });
    true
}

pub fn deposit_copper(
    entities: &mut [Entity],
    player_id: EntityId,
    amount: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    let take = amount.min(player.copper);
    if take == 0 {
        events.push(SimEvent::Toast {
            message: "No copper to deposit.".into(),
        });
        return false;
    }
    player.copper -= take;
    player.bank_copper = player.bank_copper.saturating_add(take);
    events.push(SimEvent::Toast {
        message: format!("Deposited {take}c to bank."),
    });
    true
}

pub fn withdraw_copper(
    entities: &mut [Entity],
    player_id: EntityId,
    amount: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    let take = amount.min(player.bank_copper);
    if take == 0 {
        events.push(SimEvent::Toast {
            message: "Bank vault is empty.".into(),
        });
        return false;
    }
    player.bank_copper -= take;
    player.copper = player.copper.saturating_add(take);
    events.push(SimEvent::Toast {
        message: format!("Withdrew {take}c from bank."),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn deposit_and_withdraw_roundtrip() {
        let mut entities = vec![create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0)];
        let _ = grant_into(&mut entities[0].inventory, "silverleaf", 3);
        let mut events = Vec::new();
        let bag_slot = entities[0]
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .map(|st| st.item_id == "silverleaf")
                    .unwrap_or(false)
            })
            .expect("herb in bag") as u8;
        assert!(deposit(&mut entities, 1, bag_slot, 2, &mut events));
        let bank_count: u32 = entities[0]
            .bank
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.item_id == "silverleaf")
            .map(|s| s.count)
            .sum();
        assert_eq!(bank_count, 2);
        let bank_slot = entities[0]
            .bank
            .iter()
            .position(|s| {
                s.as_ref()
                    .map(|st| st.item_id == "silverleaf")
                    .unwrap_or(false)
            })
            .unwrap() as u8;
        assert!(withdraw(&mut entities, 1, bank_slot, 2, &mut events));
    }

    #[test]
    fn copper_vault_roundtrip() {
        let mut entities = vec![create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0)];
        entities[0].copper = 100;
        let mut events = Vec::new();
        assert!(deposit_copper(&mut entities, 1, 40, &mut events));
        assert_eq!(entities[0].copper, 60);
        assert_eq!(entities[0].bank_copper, 40);
        assert!(withdraw_copper(&mut entities, 1, 25, &mut events));
        assert_eq!(entities[0].copper, 85);
        assert_eq!(entities[0].bank_copper, 15);
    }
}
