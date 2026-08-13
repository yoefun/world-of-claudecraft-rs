//! Interaction commands: talk, quests, vendor, equip, use item, loot corpse.

use crate::ecs::components::AuraInstance;
use crate::ecs::components::{dist2d, Equipment};
use crate::ecs::components::{Bags, Health, Identity, Progress};
use crate::ecs::World;
use crate::inventory::{grant_into, remove_item};
use crate::inventory::{grant_item, take_item};
use crate::quests::{accept_quest, on_talked_to, quests_for_npc, turn_in_quest};
use crate::stats::recalc_player_stats;
use crate::types::INTERACT_RANGE;
use woc_content::{item, npc, ItemEquipSlot, ItemKind};
use woc_protocol::{
    EntityId, EntityKind, EquipSlot, InteractAction, SimEvent, VendorOfferSnapshot,
};

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

fn equipment_slot_mut(equipment: &mut Equipment, slot: EquipSlot) -> &mut Option<String> {
    match slot {
        EquipSlot::MainHand => &mut equipment.main_hand,
        EquipSlot::OffHand => &mut equipment.off_hand,
        EquipSlot::Head => &mut equipment.head,
        EquipSlot::Chest => &mut equipment.chest,
        EquipSlot::Legs => &mut equipment.legs,
        EquipSlot::Feet => &mut equipment.feet,
    }
}

pub fn handle_interact(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    action: InteractAction,
    events: &mut Vec<SimEvent>,
) {
    if !world.get::<Health>(player_id).is_some_and(|h| h.alive) {
        return;
    }

    if matches!(action, InteractAction::CloseVendor) {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            bags.open_vendor_npc = None;
        }
        return;
    }

    match &action {
        InteractAction::Equip { bag_slot } => {
            equip_from_bag(world, player_id, *bag_slot, events);
            return;
        }
        InteractAction::Unequip { equip_slot } => {
            unequip_to_bag(world, player_id, *equip_slot, events);
            return;
        }
        InteractAction::UseItem { bag_slot } => {
            use_item_from_bag(world, player_id, *bag_slot, events);
            return;
        }
        // LootCorpse is handled in WorldHost (needs loot_rules for Need/Greed).
        InteractAction::LootCorpse { .. } => {
            return;
        }
        _ => {}
    }

    if dist2d(world, player_id, target_id)
        .map(|d| d > INTERACT_RANGE)
        .unwrap_or(true)
    {
        events.push(SimEvent::Toast {
            message: "Too far away.".into(),
        });
        return;
    }

    match action {
        InteractAction::Talk => talk(world, player_id, target_id, events),
        InteractAction::AcceptQuest { quest_id } => {
            let template = world
                .get::<Identity>(target_id)
                .and_then(|i| i.template_id.clone());
            if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
                return;
            }
            if accept_quest(world, player_id, &quest_id, events) {
                if let Some(tid) = template.as_deref() {
                    on_talked_to(world, player_id, tid, events);
                }
            }
        }
        InteractAction::TurnInQuest { quest_id } => {
            if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
                return;
            }
            let _ = turn_in_quest(world, player_id, &quest_id, events);
        }
        InteractAction::Buy { item_id, count } => {
            buy(world, player_id, target_id, &item_id, count, events);
        }
        InteractAction::Sell { bag_slot, count } => {
            sell(world, player_id, target_id, bag_slot, count, events);
        }
        _ => {}
    }
}

fn talk(world: &mut World, player_id: EntityId, target_id: EntityId, events: &mut Vec<SimEvent>) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let def = npc(&template_id);
    let text = def
        .map(|d| d.greeting.to_string())
        .unwrap_or_else(|| "...".into());
    events.push(SimEvent::NpcDialog {
        player: player_id,
        npc_id: target_id,
        text: text.clone(),
    });
    events.push(SimEvent::Toast { message: text });

    if let Some(d) = def {
        if d.is_vendor {
            if let Some(bags) = world.get_mut::<Bags>(player_id) {
                bags.open_vendor_npc = Some(target_id);
            }
            events.push(SimEvent::VendorOpen {
                player: player_id,
                npc_id: target_id,
            });
        }
        on_talked_to(world, player_id, &template_id, events);
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
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    item_id: &str,
    count: u32,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) || count == 0 {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
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
    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    if copper < price {
        events.push(SimEvent::Toast {
            message: "Not enough copper.".into(),
        });
        return;
    }
    if grant_item(world, player_id, item_id, count, events).is_err() {
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper -= price;
    }
    crate::quests::on_inventory_changed(world, player_id, events);
}

fn sell(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) || count == 0 {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_vendor {
        return;
    }
    let slot = bag_slot as usize;
    let stack = world
        .get::<Bags>(player_id)
        .and_then(|b| b.inventory.get(slot))
        .and_then(|s| s.clone());
    let Some(stack) = stack else {
        return;
    };
    let take = count.min(stack.count);
    let Some(idef) = item(&stack.item_id) else {
        return;
    };
    if take_item(world, player_id, &stack.item_id, take, events).is_err() {
        return;
    }
    let _ = ndef;
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p
            .copper
            .saturating_add(idef.vendor_sell.saturating_mul(take));
    }
    crate::quests::on_inventory_changed(world, player_id, events);
}

fn equip_from_bag(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    events: &mut Vec<SimEvent>,
) {
    let slot = bag_slot as usize;
    let stack = world
        .get::<Bags>(player_id)
        .and_then(|b| b.inventory.get(slot))
        .and_then(|s| s.clone());
    let Some(stack) = stack else {
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
    let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
    if level < idef.level_req {
        events.push(SimEvent::Toast {
            message: format!("Requires level {}.", idef.level_req),
        });
        return;
    }
    let equip_slot = to_protocol_slot(content_slot);
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        if !remove_item(&mut bags.inventory, &stack.item_id, 1) {
            return;
        }
        let previous =
            equipment_slot_mut(&mut bags.equipment, equip_slot).replace(stack.item_id.clone());
        if let Some(prev) = previous {
            let _ = grant_into(&mut bags.inventory, &prev, 1);
        }
    }
    recalc_player_stats(world, player_id);
    events.push(SimEvent::Equipped {
        player: player_id,
        item_id: stack.item_id,
        slot: equip_slot,
    });
}

fn unequip_to_bag(
    world: &mut World,
    player_id: EntityId,
    equip_slot: EquipSlot,
    events: &mut Vec<SimEvent>,
) {
    let item_id = world
        .get_mut::<Bags>(player_id)
        .and_then(|b| equipment_slot_mut(&mut b.equipment, equip_slot).take());
    let Some(item_id) = item_id else {
        return;
    };
    let mut restored = false;
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        if grant_into(&mut bags.inventory, &item_id, 1) {
            restored = true;
        } else {
            *equipment_slot_mut(&mut bags.equipment, equip_slot) = Some(item_id.clone());
        }
    }
    if !restored {
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    recalc_player_stats(world, player_id);
}

fn use_item_from_bag(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    events: &mut Vec<SimEvent>,
) {
    let slot = bag_slot as usize;
    let stack = world
        .get::<Bags>(player_id)
        .and_then(|b| b.inventory.get(slot))
        .and_then(|s| s.clone());
    let Some(stack) = stack else {
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
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        if !remove_item(&mut bags.inventory, &stack.item_id, 1) {
            return;
        }
    }
    let before = world.get::<Health>(player_id).map(|h| h.hp).unwrap_or(0.0);
    let hp_max = world
        .get::<Health>(player_id)
        .map(|h| h.hp_max)
        .unwrap_or(0.0);
    if let Some(h) = world.get_mut::<Health>(player_id) {
        h.hp = (h.hp + idef.heal_hp).min(hp_max);
    }
    let after = world
        .get::<Health>(player_id)
        .map(|h| h.hp)
        .unwrap_or(before);
    let healed = after - before;
    let hot_tick = (idef.heal_hp * 0.15).max(2.0);
    crate::combat::apply_aura(
        world,
        player_id,
        AuraInstance {
            id: "consumable_renew".into(),
            remaining: 3.0,
            stacks: 1,
            tick_timer: 1.0,
            tick_interval: 1.0,
            tick_damage: 0.0,
            tick_heal: hot_tick,
            source: player_id,
        },
        events,
    );
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id: stack.item_id.clone(),
        count: 1,
    });
    events.push(SimEvent::Toast {
        message: format!("You restore {:.0} health.", healed),
    });
    crate::quests::on_inventory_changed(world, player_id, events);
}

pub fn vendor_snapshot(world: &World, player_id: EntityId) -> Option<woc_protocol::VendorSnapshot> {
    let npc_id = world.get::<Bags>(player_id)?.open_vendor_npc?;
    let npc_e = world.get::<Identity>(npc_id)?;
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
    use crate::ecs::components::{Bags, Health};
    use crate::inventory::{count_item, grant_into};
    use woc_content::PlayerClass;

    fn bag_slot_of(world: &World, player_id: EntityId, item_id: &str) -> u8 {
        world
            .get::<Bags>(player_id)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == item_id))
            .expect("item in bag") as u8
    }

    #[test]
    fn use_consumable_restores_hp() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        let before_count = count_item(&world.get::<Bags>(1).unwrap().inventory, "baked_bread");
        assert!(before_count >= 1);
        let before = {
            let h = world.get_mut::<Health>(1).unwrap();
            h.hp = h.hp_max * 0.25;
            h.hp
        };
        let slot = bag_slot_of(&world, 1, "baked_bread");
        let mut events = Vec::new();
        use_item_from_bag(&mut world, 1, slot, &mut events);
        assert!(world.get::<Health>(1).unwrap().hp > before);
        assert_eq!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "baked_bread"),
            before_count - 1
        );
    }

    #[test]
    fn travelers_ration_heals() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 10.0;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "travelers_ration", 1));
        }
        let slot = bag_slot_of(&world, 1, "travelers_ration");
        let mut events = Vec::new();
        use_item_from_bag(&mut world, 1, slot, &mut events);
        let expected = (10.0_f32 + 80.0).min(world.get::<Health>(1).unwrap().hp_max);
        assert!((world.get::<Health>(1).unwrap().hp - expected).abs() < 1e-3);
    }

    #[test]
    fn equip_and_unequip_gear() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Armored", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "recruit_cap", 1));
            assert!(grant_into(&mut bags.inventory, "wooden_buckler", 1));
        }
        let mut events = Vec::new();
        let cap = bag_slot_of(&world, 1, "recruit_cap");
        equip_from_bag(&mut world, 1, cap, &mut events);
        let shield = bag_slot_of(&world, 1, "wooden_buckler");
        equip_from_bag(&mut world, 1, shield, &mut events);
        assert_eq!(
            world.get::<Bags>(1).unwrap().equipment.head.as_deref(),
            Some("recruit_cap")
        );
        assert_eq!(
            world.get::<Bags>(1).unwrap().equipment.off_hand.as_deref(),
            Some("wooden_buckler")
        );
        unequip_to_bag(&mut world, 1, EquipSlot::Head, &mut events);
        assert!(world.get::<Bags>(1).unwrap().equipment.head.is_none());
    }

    #[test]
    fn refuse_low_level_equip() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Noob", PlayerClass::Warrior, 0.0, 0.0);
        assert_eq!(world.get::<Health>(1).unwrap().level, 1);
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "veteran_helm", 1));
        }
        let slot = bag_slot_of(&world, 1, "veteran_helm");
        let mut events = Vec::new();
        equip_from_bag(&mut world, 1, slot, &mut events);
        assert!(world.get::<Bags>(1).unwrap().equipment.head.is_none());
        assert_eq!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "veteran_helm"),
            1
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message.contains("Requires level")
        )));
    }

    #[test]
    fn use_item_via_interact_action() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Hungry", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(h) = world.get_mut::<Health>(1) {
            h.hp = 5.0;
        }
        if let Some(bags) = world.get_mut::<Bags>(1) {
            assert!(grant_into(&mut bags.inventory, "baked_bread", 1));
        }
        let slot = bag_slot_of(&world, 1, "baked_bread");
        let mut events = Vec::new();
        handle_interact(
            &mut world,
            1,
            1,
            InteractAction::UseItem { bag_slot: slot },
            &mut events,
        );
        assert!(world.get::<Health>(1).unwrap().hp > 5.0);
    }
}
