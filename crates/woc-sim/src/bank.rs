//! Personal bank deposit / withdraw (items + copper vault).

use crate::ecs::components::{Bags, Bank, Progress};
use crate::ecs::World;
use crate::inventory::{grant_stack, take_from_slot};
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
    let Some(taken) = (if let Some(bags) = world.get_mut::<Bags>(player_id) {
        take_from_slot(&mut bags.inventory, bag_slot, take)
    } else {
        None
    }) else {
        return false;
    };
    let bank_full = match world.get_mut::<Bank>(player_id) {
        Some(bank) => !grant_stack(&mut bank.bank, taken.clone()),
        None => true,
    };
    if bank_full {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            let _ = grant_stack(&mut bags.inventory, taken);
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
    let Some(taken) = (if let Some(bank) = world.get_mut::<Bank>(player_id) {
        take_from_slot(&mut bank.bank, bank_slot, take)
    } else {
        None
    }) else {
        return false;
    };
    let bags_full = match world.get_mut::<Bags>(player_id) {
        Some(bags) => !grant_stack(&mut bags.inventory, taken.clone()),
        None => true,
    };
    if bags_full {
        if let Some(bank) = world.get_mut::<Bank>(player_id) {
            let _ = grant_stack(&mut bank.bank, taken);
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

    #[test]
    fn deposit_preserves_bound_flag() {
        use crate::ecs::components::InvStack;
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            bags.inventory[0] = Some(InvStack {
                item_id: "worn_sword".into(),
                count: 1,
                durability: Some(7),
                enchant_id: None,
                quality: None,
                bound: true,
            });
        }
        let mut events = Vec::new();
        assert!(deposit(&mut world, 1, 0, 1, &mut events));
        let stored = world
            .get::<Bank>(1)
            .unwrap()
            .bank
            .iter()
            .flatten()
            .find(|s| s.item_id == "worn_sword")
            .unwrap();
        assert!(stored.bound);
        assert_eq!(stored.durability, Some(7));
    }

    #[test]
    fn interact_bank_requires_banker_session() {
        use crate::ecs::components::{Identity, Transform};
        use woc_protocol::{EntityId, InteractAction, WorldHost};
        let mut sim = crate::sim::Sim::new_eastbrook("Ada", PlayerClass::Warrior);
        let pid = sim.player_id;
        if let Some(bags) = sim.world.get_mut::<Bags>(pid) {
            assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
        }
        let slot = sim
            .world
            .get::<Bags>(pid)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "silverleaf"))
            .unwrap() as u8;
        WorldHost::interact(
            &mut sim,
            pid,
            0,
            InteractAction::BankDeposit {
                bag_slot: slot,
                count: 1,
            },
        );
        assert!(
            sim.world.get::<Bank>(pid).is_none()
                || sim
                    .world
                    .get::<Bank>(pid)
                    .unwrap()
                    .bank
                    .iter()
                    .flatten()
                    .next()
                    .is_none()
        );
        assert!(sim.events.iter().any(|e| matches!(
            e,
            woc_protocol::SimEvent::Toast { message } if message == "Talk to a banker first."
        )));

        let holme = sim
            .world
            .ids::<Identity>()
            .into_iter()
            .find(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some("banker_holme")
            })
            .expect("banker_holme");
        let _nid: EntityId = holme;
        if let Some(nt) = sim.world.get::<Transform>(holme).cloned() {
            if let Some(p) = sim.world.get_mut::<Transform>(pid) {
                p.x = nt.x;
                p.z = nt.z;
            }
        }
        WorldHost::interact(&mut sim, pid, holme, InteractAction::Talk);
        sim.events.clear();
        WorldHost::interact(
            &mut sim,
            pid,
            holme,
            InteractAction::BankDeposit {
                bag_slot: slot,
                count: 1,
            },
        );
        assert!(sim
            .world
            .get::<Bank>(pid)
            .unwrap()
            .bank
            .iter()
            .flatten()
            .any(|s| s.item_id == "silverleaf"));
    }
}
