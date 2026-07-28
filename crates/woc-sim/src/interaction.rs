//! Interaction commands: talk, quests, vendor, equip, use item, loot corpse.

use crate::entity::{grant_into, remove_item, Entity};
use crate::inventory::{grant_item, take_item};
use crate::quests::{accept_quest, on_talked_to, quests_for_npc, turn_in_quest};
use crate::stats::recalc_player_stats;
use crate::types::INTERACT_RANGE;
use woc_content::{item, npc, ItemEquipSlot, ItemKind};
use woc_protocol::{
    EntityId, EntityKind, EquipSlot, InteractAction, SimEvent, VendorOfferSnapshot,
};

pub fn dist2d(a: &Entity, b: &Entity) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn to_protocol_slot(slot: ItemEquipSlot) -> EquipSlot {
    match slot {
        ItemEquipSlot::MainHand => EquipSlot::MainHand,
        ItemEquipSlot::OffHand => EquipSlot::OffHand,
        ItemEquipSlot::Head => EquipSlot::Head,
        ItemEquipSlot::Chest => EquipSlot::Chest,
        ItemEquipSlot::Legs => EquipSlot::Legs,
        ItemEquipSlot::Feet => EquipSlot::Feet,
    }
}

fn equipment_slot_mut(player: &mut Entity, slot: EquipSlot) -> &mut Option<String> {
    match slot {
        EquipSlot::MainHand => &mut player.equipment.main_hand,
        EquipSlot::OffHand => &mut player.equipment.off_hand,
        EquipSlot::Head => &mut player.equipment.head,
        EquipSlot::Chest => &mut player.equipment.chest,
        EquipSlot::Legs => &mut player.equipment.legs,
        EquipSlot::Feet => &mut player.equipment.feet,
    }
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

    if matches!(action, InteractAction::CloseVendor) {
        entities[pi].open_vendor_npc = None;
        return;
    }

    match &action {
        InteractAction::Equip { bag_slot } => {
            equip_from_bag(&mut entities[pi], *bag_slot, events);
            return;
        }
        InteractAction::Unequip { equip_slot } => {
            unequip_to_bag(&mut entities[pi], *equip_slot, events);
            return;
        }
        InteractAction::UseItem { bag_slot } => {
            use_item_from_bag(&mut entities[pi], *bag_slot, events);
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
    let Some(content_slot) = idef.equip_slot else {
        events.push(SimEvent::Toast {
            message: "Cannot equip that.".into(),
        });
        return;
    };
    if player.level < idef.level_req {
        events.push(SimEvent::Toast {
            message: format!("Requires level {}.", idef.level_req),
        });
        return;
    }
    let equip_slot = to_protocol_slot(content_slot);
    if !remove_item(&mut player.inventory, &stack.item_id, 1) {
        return;
    }
    let previous = equipment_slot_mut(player, equip_slot).replace(stack.item_id.clone());
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
    let item_id = equipment_slot_mut(player, equip_slot).take();
    let Some(item_id) = item_id else {
        return;
    };
    if !grant_into(&mut player.inventory, &item_id, 1) {
        *equipment_slot_mut(player, equip_slot) = Some(item_id);
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    recalc_player_stats(player);
}

fn use_item_from_bag(player: &mut Entity, bag_slot: u8, events: &mut Vec<SimEvent>) {
    let slot = bag_slot as usize;
    let Some(Some(stack)) = player.inventory.get(slot).cloned() else {
        return;
    };
    let Some(idef) = item(&stack.item_id) else {
        return;
    };
    if idef.kind != ItemKind::Consumable || idef.heal_hp <= 0.0 {
        events.push(SimEvent::Toast {
            message: "Cannot use that.".into(),
        });
        return;
    }
    if !remove_item(&mut player.inventory, &stack.item_id, 1) {
        return;
    }
    let before = player.hp;
    player.hp = (player.hp + idef.heal_hp).min(player.hp_max);
    let healed = player.hp - before;
    // Short HoT linger after consumable (≈3s at 4 HP/tick).
    let hot_tick = (idef.heal_hp * 0.15).max(2.0);
    crate::combat::apply_aura(
        player,
        crate::entity::AuraInstance {
            id: "consumable_renew".into(),
            remaining: 3.0,
            stacks: 1,
            tick_timer: 1.0,
            tick_interval: 1.0,
            tick_damage: 0.0,
            tick_heal: hot_tick,
            source: player.id,
        },
        events,
    );
    events.push(SimEvent::ItemLost {
        player: player.id,
        item_id: stack.item_id.clone(),
        count: 1,
    });
    events.push(SimEvent::Toast {
        message: format!("You restore {:.0} health.", healed),
    });
    crate::quests::on_inventory_changed(player, events);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{create_player, grant_into};
    use woc_content::PlayerClass;

    fn bag_slot_of(player: &Entity, item_id: &str) -> u8 {
        player
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == item_id))
            .expect("item in bag") as u8
    }

    #[test]
    fn use_consumable_restores_hp() {
        let mut player = create_player(1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        player.hp = player.hp_max * 0.25;
        let before = player.hp;
        let before_count = crate::entity::count_item(&player.inventory, "baked_bread");
        assert!(before_count >= 1, "warrior starts with bread");
        let slot = bag_slot_of(&player, "baked_bread");
        let mut events = Vec::new();
        use_item_from_bag(&mut player, slot, &mut events);
        assert!(player.hp > before);
        assert!(player.hp <= player.hp_max);
        assert_eq!(
            crate::entity::count_item(&player.inventory, "baked_bread"),
            before_count - 1
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::ItemLost { .. })));
    }

    #[test]
    fn travelers_ration_heals() {
        let mut player = create_player(1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        player.hp = 10.0;
        assert!(grant_into(&mut player.inventory, "travelers_ration", 1));
        let slot = bag_slot_of(&player, "travelers_ration");
        let mut events = Vec::new();
        use_item_from_bag(&mut player, slot, &mut events);
        let expected = (10.0_f32 + 80.0).min(player.hp_max);
        assert!((player.hp - expected).abs() < 1e-3);
    }

    #[test]
    fn refuse_low_level_equip() {
        let mut player = create_player(1, "Noob", PlayerClass::Warrior, 0.0, 0.0);
        assert_eq!(player.level, 1);
        assert!(grant_into(&mut player.inventory, "veteran_helm", 1));
        let slot = bag_slot_of(&player, "veteran_helm");
        let mut events = Vec::new();
        equip_from_bag(&mut player, slot, &mut events);
        assert!(player.equipment.head.is_none());
        assert_eq!(
            crate::entity::count_item(&player.inventory, "veteran_helm"),
            1
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message.contains("Requires level")
        )));
    }

    #[test]
    fn equip_head_and_offhand() {
        let mut player = create_player(1, "Geared", PlayerClass::Warrior, 0.0, 0.0);
        assert!(grant_into(&mut player.inventory, "recruit_cap", 1));
        assert!(grant_into(&mut player.inventory, "wooden_buckler", 1));
        let mut events = Vec::new();
        let head_slot = bag_slot_of(&player, "recruit_cap");
        equip_from_bag(&mut player, head_slot, &mut events);
        let oh_slot = bag_slot_of(&player, "wooden_buckler");
        equip_from_bag(&mut player, oh_slot, &mut events);
        assert_eq!(player.equipment.head.as_deref(), Some("recruit_cap"));
        assert_eq!(player.equipment.off_hand.as_deref(), Some("wooden_buckler"));
    }

    #[test]
    fn use_item_via_interact_action() {
        let mut player = create_player(1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        player.hp = 5.0;
        assert!(grant_into(&mut player.inventory, "baked_bread", 1));
        let slot = bag_slot_of(&player, "baked_bread");
        let pid = player.id;
        let mut entities = vec![player];
        let mut events = Vec::new();
        handle_interact(
            &mut entities,
            pid,
            pid,
            InteractAction::UseItem { bag_slot: slot },
            &mut events,
        );
        assert!(entities[0].hp > 5.0);
    }
}
