//! Personal bank deposit / withdraw (items + copper vault).

use crate::ecs::components::{Bags, Bank, Progress};
use crate::ecs::World;
use crate::inventory::{put_stack, take_from_slot};
use crate::types::BANK_SLOTS;
use woc_protocol::{EntityId, SimEvent};

fn ensure_bank(world: &mut World, player_id: EntityId) {
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
}

pub fn deposit(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    ensure_bank(world, player_id);
    let Some(taken) = world.get_mut::<Bags>(player_id).and_then(|b| {
        take_from_slot(&mut b.inventory, bag_slot as usize, count)
    }) else {
        events.push(SimEvent::Toast {
            message: "Empty bag slot.".into(),
        });
        return false;
    };
    let item_id = taken.item_id.clone();
    let n = taken.count;
    let bank_full = match world.get_mut::<Bank>(player_id) {
        Some(bank) => put_stack(&mut bank.bank, taken.clone()).is_err(),
        None => true,
    };
    if bank_full {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            let _ = put_stack(&mut bags.inventory, taken);
        }
        events.push(SimEvent::Toast {
            message: "Bank is full.".into(),
        });
        return false;
    }
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id,
        count: n,
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
    let Some(taken) = world.get_mut::<Bank>(player_id).and_then(|b| {
        take_from_slot(&mut b.bank, bank_slot as usize, count)
    }) else {
        events.push(SimEvent::Toast {
            message: "Empty bank slot.".into(),
        });
        return false;
    };
    let item_id = taken.item_id.clone();
    let n = taken.count;
    let bags_full = match world.get_mut::<Bags>(player_id) {
        Some(bags) => put_stack(&mut bags.inventory, taken.clone()).is_err(),
        None => true,
    };
    if bags_full {
        if let Some(bank) = world.get_mut::<Bank>(player_id) {
            let _ = put_stack(&mut bank.bank, taken);
        }
        events.push(SimEvent::Toast {
            message: "Bags are full.".into(),
        });
        return false;
    }
    events.push(SimEvent::ItemGained {
        player: player_id,
        item_id,
        count: n,
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
    fn deposit_preserves_worn_enchanted_sword() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            let _ = grant_into(&mut bags.inventory, "worn_sword", 1);
        }
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|x| x.item_id == "worn_sword"))
            .unwrap();
        if let Some(bags) = world.get_mut::<Bags>(1) {
            if let Some(st) = bags.inventory[slot].as_mut() {
                st.durability = Some(12);
                st.enchant_id = Some("coarse_sharpening".into());
            }
        }
        let mut events = Vec::new();
        assert!(deposit(&mut world, 1, slot as u8, 1, &mut events));
        let stored = world
            .get::<Bank>(1)
            .unwrap()
            .bank
            .iter()
            .flatten()
            .find(|s| s.item_id == "worn_sword")
            .unwrap();
        assert_eq!(stored.durability, Some(12));
        assert_eq!(stored.enchant_id.as_deref(), Some("coarse_sharpening"));
    }

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
