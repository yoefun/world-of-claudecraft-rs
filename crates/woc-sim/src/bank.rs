//! Personal bank deposit / withdraw (items + copper vault).

use crate::ecs::components::{Bags, Bank, Progress};
use crate::ecs::World;
use crate::inventory::{grant_into, remove_item};
use crate::types::BANK_SLOTS;
use woc_protocol::{EntityId, SimEvent};

pub fn deposit(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    if world.get::<Bags>(player_id).is_none() {
        return false;
    }
    if let Some(bank) = world.get_mut::<Bank>(player_id) {
        if bank.bank.len() < BANK_SLOTS {
            bank.bank.resize(BANK_SLOTS, None);
        }
    } else {
        world.insert(
            player_id,
            Bank {
                bank: vec![None; BANK_SLOTS],
                bank_copper: 0,
            },
        );
    }
    let slot = bag_slot as usize;
    let stack = world
        .get::<Bags>(player_id)
        .and_then(|b| b.inventory.get(slot))
        .and_then(|s| s.clone());
    let Some(stack) = stack else {
        events.push(SimEvent::Toast {
            message: "Empty bag slot.".into(),
        });
        return false;
    };
    let take = count.min(stack.count).max(1);
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        if !remove_item(&mut bags.inventory, &stack.item_id, take) {
            return false;
        }
    }
    let bank_full = match world.get_mut::<Bank>(player_id) {
        Some(bank) => !grant_into(&mut bank.bank, &stack.item_id, take),
        None => true,
    };
    if bank_full {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            let _ = grant_into(&mut bags.inventory, &stack.item_id, take);
        }
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
    world: &mut World,
    player_id: EntityId,
    bank_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    if world.get::<Bags>(player_id).is_none() {
        return false;
    }
    let slot = bank_slot as usize;
    let stack = world
        .get::<Bank>(player_id)
        .and_then(|b| b.bank.get(slot))
        .and_then(|s| s.clone());
    let Some(stack) = stack else {
        events.push(SimEvent::Toast {
            message: "Empty bank slot.".into(),
        });
        return false;
    };
    let take = count.min(stack.count).max(1);
    if let Some(bank) = world.get_mut::<Bank>(player_id) {
        if !remove_item(&mut bank.bank, &stack.item_id, take) {
            return false;
        }
    }
    let bags_full = match world.get_mut::<Bags>(player_id) {
        Some(bags) => !grant_into(&mut bags.inventory, &stack.item_id, take),
        None => true,
    };
    if bags_full {
        if let Some(bank) = world.get_mut::<Bank>(player_id) {
            let _ = grant_into(&mut bank.bank, &stack.item_id, take);
        }
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
    world: &mut World,
    player_id: EntityId,
    amount: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    let take = amount.min(copper);
    if take == 0 {
        events.push(SimEvent::Toast {
            message: "No copper to deposit.".into(),
        });
        return false;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper -= take;
    }
    if let Some(bank) = world.get_mut::<Bank>(player_id) {
        bank.bank_copper = bank.bank_copper.saturating_add(take);
    } else {
        world.insert(
            player_id,
            Bank {
                bank: vec![None; BANK_SLOTS],
                bank_copper: take,
            },
        );
    }
    events.push(SimEvent::Toast {
        message: format!("Deposited {take}c to bank."),
    });
    true
}

pub fn withdraw_copper(
    world: &mut World,
    player_id: EntityId,
    amount: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    let vault = world
        .get::<Bank>(player_id)
        .map(|b| b.bank_copper)
        .unwrap_or(0);
    let take = amount.min(vault);
    if take == 0 {
        events.push(SimEvent::Toast {
            message: "Bank vault is empty.".into(),
        });
        return false;
    }
    if let Some(bank) = world.get_mut::<Bank>(player_id) {
        bank.bank_copper -= take;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_add(take);
    }
    events.push(SimEvent::Toast {
        message: format!("Withdrew {take}c from bank."),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Bank, Progress};
    use crate::inventory::grant_into;
    use woc_content::PlayerClass;

    #[test]
    fn deposit_and_withdraw_round_trip() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            let _ = grant_into(&mut bags.inventory, "silverleaf", 3);
        }
        let mut events = Vec::new();
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().map(|x| x.item_id.as_str()) == Some("silverleaf"))
            .unwrap();
        assert!(deposit(&mut world, 1, slot as u8, 2, &mut events));
        assert_eq!(
            world
                .get::<Bank>(1)
                .unwrap()
                .bank
                .iter()
                .flatten()
                .find(|s| s.item_id == "silverleaf")
                .unwrap()
                .count,
            2
        );
        let bank_slot = world
            .get::<Bank>(1)
            .unwrap()
            .bank
            .iter()
            .position(|s| s.as_ref().map(|x| x.item_id.as_str()) == Some("silverleaf"))
            .unwrap();
        assert!(withdraw(&mut world, 1, bank_slot as u8, 2, &mut events));
    }

    #[test]
    fn copper_vault_roundtrip() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.copper = 100;
        }
        let mut events = Vec::new();
        assert!(deposit_copper(&mut world, 1, 40, &mut events));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 60);
        assert_eq!(world.get::<Bank>(1).unwrap().bank_copper, 40);
        assert!(withdraw_copper(&mut world, 1, 25, &mut events));
        assert_eq!(world.get::<Progress>(1).unwrap().copper, 85);
        assert_eq!(world.get::<Bank>(1).unwrap().bank_copper, 15);
    }
}
