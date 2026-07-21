//! Interaction commands: talk, quests, vendor, equip, loot corpse.

use crate::entity::{grant_into, remove_item, Entity};
use crate::inventory::{grant_item, take_item};
use crate::quests::{accept_quest, on_talked_to, quests_for_npc, turn_in_quest};
use crate::stats::recalc_player_stats;
use crate::types::INTERACT_RANGE;
use woc_content::{item, npc, ItemKind};
use woc_protocol::{
    EntityId, EntityKind, EquipSlot, InteractAction, SimEvent, VendorOfferSnapshot,
};

pub fn dist2d(a: &Entity, b: &Entity) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

pub fn handle_interact(
    entities: &mut [Entity],
    player_id: EntityId,
    target_id: EntityId,
    action: InteractAction,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    let Some(ti) = entities.iter().position(|e| e.id == target_id) else {
        return;
    };
    if !entities[pi].alive {
        return;
    }

    // CloseVendor does not need range / target NPC.
    if matches!(action, InteractAction::CloseVendor) {
        entities[pi].open_vendor_npc = None;
        return;
    }

    // Equip/Unequip are self actions (target may be self).
    match &action {
        InteractAction::Equip { bag_slot } => {
            equip_from_bag(&mut entities[pi], *bag_slot, events);
            return;
        }
        InteractAction::Unequip { equip_slot } => {
            unequip_to_bag(&mut entities[pi], *equip_slot, events);
            return;
        }
        InteractAction::LootCorpse { target_id: corpse } => {
            loot_corpse(entities, player_id, *corpse, events);
            return;
        }
        _ => {}
    }

    if dist2d(&entities[pi], &entities[ti]) > INTERACT_RANGE {
        events.push(SimEvent::Toast {
            message: "Too far away.".into(),
        });
        return;
    }

    match action {
        InteractAction::Talk => talk(entities, pi, ti, events),
        InteractAction::AcceptQuest { quest_id } => {
            let template = entities[ti].template_id.clone();
            if entities[ti].kind != EntityKind::Npc {
                return;
            }
            if accept_quest(&mut entities[pi], &quest_id, events) {
                if let Some(tid) = template.as_deref() {
                    on_talked_to(&mut entities[pi], tid, events);
                }
            }
        }
        InteractAction::TurnInQuest { quest_id } => {
            if entities[ti].kind != EntityKind::Npc {
                return;
            }
            let _ = turn_in_quest(&mut entities[pi], &quest_id, events);
        }
        InteractAction::Buy { item_id, count } => {
            buy(entities, pi, ti, &item_id, count, events);
        }
        InteractAction::Sell { bag_slot, count } => {
            sell(entities, pi, ti, bag_slot, count, events);
        }
        _ => {}
    }
}

fn talk(entities: &mut [Entity], pi: usize, ti: usize, events: &mut Vec<SimEvent>) {
    if entities[ti].kind != EntityKind::Npc {
        return;
    }
    let template_id = entities[ti].template_id.clone().unwrap_or_default();
    let def = npc(&template_id);
    let text = def
        .map(|d| d.greeting.to_string())
        .unwrap_or_else(|| "...".into());
    let npc_eid = entities[ti].id;
    events.push(SimEvent::NpcDialog {
        player: entities[pi].id,
        npc_id: npc_eid,
        text: text.clone(),
    });
    events.push(SimEvent::Toast { message: text });

    if let Some(d) = def {
        if d.is_vendor {
            entities[pi].open_vendor_npc = Some(npc_eid);
            events.push(SimEvent::VendorOpen {
                player: entities[pi].id,
                npc_id: npc_eid,
            });
        }
        on_talked_to(&mut entities[pi], &template_id, events);
        let available = quests_for_npc(&template_id);
        if !available.is_empty() && d.is_quest_giver {
            let names: Vec<&str> = available.iter().map(|q| q.name).collect();
            events.push(SimEvent::Toast {
                message: format!("Quests: {}", names.join(", ")),
            });
        }
    }
}

fn buy(
    entities: &mut [Entity],
    pi: usize,
    ti: usize,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) {
    if entities[ti].kind != EntityKind::Npc || count == 0 {
        return;
    }
    let template_id = entities[ti].template_id.clone().unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_vendor || !ndef.vendor_stock.iter().any(|o| o.item_id == item_id) {
        return;
    }
    let Some(idef) = item(item_id) else {
        return;
    };
    let price = idef.vendor_buy.saturating_mul(count);
    if entities[pi].copper < price {
        events.push(SimEvent::Toast {
            message: "Not enough copper.".into(),
        });
        return;
    }
    if grant_item(&mut entities[pi], item_id, count, events).is_err() {
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    entities[pi].copper -= price;
    crate::quests::on_inventory_changed(&mut entities[pi], events);
}

fn sell(
    entities: &mut [Entity],
    pi: usize,
    ti: usize,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) {
    if entities[ti].kind != EntityKind::Npc || count == 0 {
        return;
    }
    let template_id = entities[ti].template_id.clone().unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_vendor {
        return;
    }
    let slot = bag_slot as usize;
    let Some(Some(stack)) = entities[pi].inventory.get(slot).cloned() else {
        return;
    };
    let take = count.min(stack.count);
    let Some(idef) = item(&stack.item_id) else {
        return;
    };
    if take_item(&mut entities[pi], &stack.item_id, take, events).is_err() {
        return;
    }
    // Prefer removing from the chosen slot when possible — take_item already removed.
    let _ = ndef;
    entities[pi].copper = entities[pi]
        .copper
        .saturating_add(idef.vendor_sell.saturating_mul(take));
    crate::quests::on_inventory_changed(&mut entities[pi], events);
}

fn equip_from_bag(player: &mut Entity, bag_slot: u8, events: &mut Vec<SimEvent>) {
    let slot = bag_slot as usize;
    let Some(Some(stack)) = player.inventory.get(slot).cloned() else {
        return;
    };
    let Some(idef) = item(&stack.item_id) else {
        return;
    };
    let equip_slot = match idef.kind {
        ItemKind::Weapon => EquipSlot::MainHand,
        ItemKind::Armor => EquipSlot::Chest,
        _ => {
            events.push(SimEvent::Toast {
                message: "Cannot equip that.".into(),
            });
            return;
        }
    };
    // Remove one from bag.
    if !remove_item(&mut player.inventory, &stack.item_id, 1) {
        return;
    }
    let previous = match equip_slot {
        EquipSlot::MainHand => player.equipment.main_hand.replace(stack.item_id.clone()),
        EquipSlot::Chest => player.equipment.chest.replace(stack.item_id.clone()),
        EquipSlot::OffHand => player.equipment.off_hand.replace(stack.item_id.clone()),
    };
    if let Some(prev) = previous {
        let _ = grant_into(&mut player.inventory, &prev, 1);
    }
    recalc_player_stats(player);
    events.push(SimEvent::Equipped {
        player: player.id,
        item_id: stack.item_id,
        slot: equip_slot,
    });
}

fn unequip_to_bag(player: &mut Entity, equip_slot: EquipSlot, events: &mut Vec<SimEvent>) {
    let item_id = match equip_slot {
        EquipSlot::MainHand => player.equipment.main_hand.take(),
        EquipSlot::OffHand => player.equipment.off_hand.take(),
        EquipSlot::Chest => player.equipment.chest.take(),
    };
    let Some(item_id) = item_id else {
        return;
    };
    if !grant_into(&mut player.inventory, &item_id, 1) {
        // Put back if bag full.
        match equip_slot {
            EquipSlot::MainHand => player.equipment.main_hand = Some(item_id),
            EquipSlot::OffHand => player.equipment.off_hand = Some(item_id),
            EquipSlot::Chest => player.equipment.chest = Some(item_id),
        }
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    recalc_player_stats(player);
}

fn loot_corpse(
    entities: &mut [Entity],
    player_id: EntityId,
    corpse_id: EntityId,
    events: &mut Vec<SimEvent>,
) {
    let Some(pi) = entities.iter().position(|e| e.id == player_id) else {
        return;
    };
    let Some(ci) = entities.iter().position(|e| e.id == corpse_id) else {
        return;
    };
    if entities[ci].alive || entities[ci].kind != EntityKind::Mob {
        return;
    }
    if dist2d(&entities[pi], &entities[ci]) > INTERACT_RANGE {
        return;
    }
    // Framework: corpse loot already spawned as ground loot on death.
    let _ = events;
}

pub fn vendor_snapshot(
    entities: &[Entity],
    player: &Entity,
) -> Option<woc_protocol::VendorSnapshot> {
    let npc_id = player.open_vendor_npc?;
    let npc_e = entities.iter().find(|e| e.id == npc_id)?;
    let template = npc_e.template_id.as_deref()?;
    let def = npc(template)?;
    let stock = def
        .vendor_stock
        .iter()
        .filter_map(|o| {
            let price = item(o.item_id)?.vendor_buy;
            Some(VendorOfferSnapshot {
                item_id: o.item_id.to_string(),
                count: o.count,
                price,
            })
        })
        .collect();
    Some(woc_protocol::VendorSnapshot {
        npc_id,
        npc_name: npc_e.name.clone(),
        stock,
    })
}
