//! Interaction commands: talk, quests, vendor, equip, use item, loot corpse.

use crate::ecs::components::AuraInstance;
use crate::ecs::components::{dist2d, Equipment, EquipmentWear, Hearth, InvStack, Transform};
use crate::ecs::components::{Bags, BuybackEntry, ClassKit, Health, Identity, Progress};
use crate::ecs::World;
use crate::inventory::remove_item;
use crate::inventory::{grant_item, take_item};
use crate::quests::{
    accept_quest, npc_quest_offers, on_talked_to, quest_log_entries, turn_in_quest,
};
use crate::stats::recalc_player_stats;
use crate::types::INTERACT_RANGE;
use woc_content::{
    item, known_abilities_at_level, npc, ItemEquipSlot, ItemKind, NpcDef, NpcService,
};
use woc_protocol::{
    BuybackSnapshot, EntityId, EntityKind, EquipSlot, InteractAction, NpcSessionSnapshot, SimEvent,
    VendorOfferSnapshot,
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

fn equipment_slot(equipment: &Equipment, slot: EquipSlot) -> &Option<String> {
    match slot {
        EquipSlot::MainHand => &equipment.main_hand,
        EquipSlot::OffHand => &equipment.off_hand,
        EquipSlot::Head => &equipment.head,
        EquipSlot::Chest => &equipment.chest,
        EquipSlot::Legs => &equipment.legs,
        EquipSlot::Feet => &equipment.feet,
    }
}

fn equipment_wear_slot_mut(wear: &mut EquipmentWear, slot: EquipSlot) -> &mut Option<u32> {
    match slot {
        EquipSlot::MainHand => &mut wear.main_hand,
        EquipSlot::OffHand => &mut wear.off_hand,
        EquipSlot::Head => &mut wear.head,
        EquipSlot::Chest => &mut wear.chest,
        EquipSlot::Legs => &mut wear.legs,
        EquipSlot::Feet => &mut wear.feet,
    }
}

fn equipment_wear_slot(wear: &EquipmentWear, slot: EquipSlot) -> Option<u32> {
    match slot {
        EquipSlot::MainHand => wear.main_hand,
        EquipSlot::OffHand => wear.off_hand,
        EquipSlot::Head => wear.head,
        EquipSlot::Chest => wear.chest,
        EquipSlot::Legs => wear.legs,
        EquipSlot::Feet => wear.feet,
    }
}

const EQUIP_SLOTS: [EquipSlot; 6] = [
    EquipSlot::MainHand,
    EquipSlot::OffHand,
    EquipSlot::Head,
    EquipSlot::Chest,
    EquipSlot::Legs,
    EquipSlot::Feet,
];

fn stack_with_durability(item_id: &str, count: u32, durability: Option<u32>) -> InvStack {
    let mut stack = InvStack::new(item_id, count);
    if durability.is_some() {
        stack.durability = durability;
    }
    stack
}

pub fn repair_cost(world: &World, player_id: EntityId) -> u32 {
    let Some(bags) = world.get::<Bags>(player_id) else {
        return 0;
    };
    let mut cost = 0u32;

    for slot in EQUIP_SLOTS {
        let Some(item_id) = equipment_slot(&bags.equipment, slot).as_deref() else {
            continue;
        };
        let Some(max) = EquipmentWear::max_for_item(item_id) else {
            continue;
        };
        let current = equipment_wear_slot(&bags.equipment_wear, slot).unwrap_or(max);
        cost = cost.saturating_add(max.saturating_sub(current));
    }

    for stack in bags.inventory.iter().flatten() {
        let Some(def) = item(&stack.item_id) else {
            continue;
        };
        if def.max_durability == 0 {
            continue;
        }
        let current = stack.durability.unwrap_or(def.max_durability);
        cost = cost.saturating_add(def.max_durability.saturating_sub(current));
    }

    cost
}

pub fn handle_interact(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    action: InteractAction,
    now_tick: u64,
    events: &mut Vec<SimEvent>,
) {
    if !world.get::<Health>(player_id).is_some_and(|h| h.alive) {
        return;
    }

    if matches!(action, InteractAction::CloseVendor) {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            bags.open_vendor_npc = None;
            bags.buyback.clear();
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
            let template_id = match world.get::<Identity>(target_id) {
                Some(i) if i.kind == EntityKind::Npc => i.template_id.clone(),
                _ => return,
            };
            let Some(tid) = template_id.as_deref() else {
                return;
            };
            if accept_quest(world, player_id, &quest_id, tid, events) {
                on_talked_to(world, player_id, tid, events);
            }
        }
        InteractAction::TurnInQuest {
            quest_id,
            reward_choice,
        } => {
            if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
                return;
            }
            let Some(tid) = world
                .get::<Identity>(target_id)
                .and_then(|i| i.template_id.clone())
            else {
                return;
            };
            let _ = turn_in_quest(
                world,
                player_id,
                &quest_id,
                &tid,
                now_tick,
                reward_choice,
                events,
            );
        }
        InteractAction::Buy { item_id, count } => {
            buy(world, player_id, target_id, &item_id, count, events);
        }
        InteractAction::Sell { bag_slot, count } => {
            sell(world, player_id, target_id, bag_slot, count, events);
        }
        InteractAction::Buyback { slot } => {
            buyback(world, player_id, target_id, slot, events);
        }
        InteractAction::RepairAll => {
            repair_all(world, player_id, target_id, events);
        }
        InteractAction::TrainClass => {
            train_class(world, player_id, target_id, events);
        }
        InteractAction::BindHearth => {
            bind_hearth(world, player_id, target_id, events);
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
        if opens_npc_session(d) {
            if let Some(bags) = world.get_mut::<Bags>(player_id) {
                bags.open_vendor_npc = Some(target_id);
            }
        }
        if d.is_vendor() {
            events.push(SimEvent::VendorOpen {
                player: player_id,
                npc_id: target_id,
            });
        }
        on_talked_to(world, player_id, &template_id, events);
        let offers = npc_quest_offers(&template_id, &quest_log_entries(world, player_id));
        let mut names: Vec<&str> = offers.accept.iter().map(|q| q.name).collect();
        names.extend(offers.turn_in.iter().map(|q| q.name));
        if !names.is_empty() && d.is_quest_giver() {
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
    if !ndef.is_vendor() || !ndef.vendor_stock.iter().any(|o| o.item_id == item_id) {
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
    if !ndef.is_vendor() {
        return;
    }
    if world.get::<Bags>(player_id).and_then(|b| b.open_vendor_npc) != Some(target_id) {
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
    if idef.kind == ItemKind::Quest {
        events.push(SimEvent::Toast {
            message: "This item is needed for a quest.".into(),
        });
        return;
    }
    let copper = idef.vendor_sell.saturating_mul(take);
    let durability = stack.durability;
    if take_item(world, player_id, &stack.item_id, take, events).is_err() {
        return;
    }
    let _ = ndef;
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_add(copper);
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.buyback.push(BuybackEntry {
            item_id: stack.item_id,
            count: take,
            durability,
            copper,
        });
        if bags.buyback.len() > 6 {
            bags.buyback.remove(0);
        }
    }
    crate::quests::on_inventory_changed(world, player_id, events);
}

fn buyback(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    slot: u8,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_vendor()
        || world.get::<Bags>(player_id).and_then(|b| b.open_vendor_npc) != Some(target_id)
    {
        return;
    }

    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    let slot = slot as usize;
    let Some(entry_price) = world
        .get::<Bags>(player_id)
        .and_then(|b| b.buyback.get(slot))
        .map(|entry| entry.copper)
    else {
        return;
    };
    if copper < entry_price {
        events.push(SimEvent::Toast {
            message: "Not enough copper.".into(),
        });
        return;
    }

    let entry = {
        let Some(bags) = world.get_mut::<Bags>(player_id) else {
            return;
        };
        if slot >= bags.buyback.len() {
            return;
        }
        bags.buyback.remove(slot)
    };
    let item_id = entry.item_id.clone();
    let count = entry.count;
    let price = entry.copper;
    let durability = entry.durability;
    let inventory_before = world
        .get::<Bags>(player_id)
        .map(|bags| bags.inventory.clone())
        .unwrap_or_default();
    let granted = if durability.is_some() {
        world
            .get_mut::<Bags>(player_id)
            .and_then(|bags| bags.inventory.iter_mut().find(|slot| slot.is_none()))
            .map(|empty| {
                *empty = Some(stack_with_durability(&item_id, count, durability));
            })
            .is_some()
    } else {
        grant_item(world, player_id, &item_id, count, events).is_ok()
    };
    if !granted {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            bags.inventory = inventory_before;
            if slot <= bags.buyback.len() {
                bags.buyback.insert(slot, entry);
            } else {
                bags.buyback.push(entry);
            }
        }
        events.push(SimEvent::Toast {
            message: "Inventory full.".into(),
        });
        return;
    }
    if durability.is_some() {
        events.push(SimEvent::ItemGained {
            player: player_id,
            item_id: item_id.clone(),
            count,
        });
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_sub(price);
    }
    crate::quests::on_inventory_changed(world, player_id, events);
}

fn repair_all(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.can_repair()
        || world.get::<Bags>(player_id).and_then(|b| b.open_vendor_npc) != Some(target_id)
    {
        return;
    }

    let cost = repair_cost(world, player_id);
    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    if copper < cost {
        events.push(SimEvent::Toast {
            message: "Not enough copper.".into(),
        });
        return;
    }

    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        for slot in EQUIP_SLOTS {
            let item_id = equipment_slot(&bags.equipment, slot).clone();
            let Some(item_id) = item_id else {
                continue;
            };
            let Some(max) = EquipmentWear::max_for_item(&item_id) else {
                continue;
            };
            *equipment_wear_slot_mut(&mut bags.equipment_wear, slot) = Some(max);
        }
        for stack in bags.inventory.iter_mut().flatten() {
            let Some(def) = item(&stack.item_id) else {
                continue;
            };
            if def.max_durability > 0 {
                stack.durability = Some(def.max_durability);
            }
        }
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_sub(cost);
    }
    recalc_player_stats(world, player_id);
    events.push(SimEvent::Toast {
        message: format!("Repaired for {cost} copper."),
    });
}

fn train_class(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_class_trainer() {
        return;
    }

    let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
    let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
    let Some(class) = class else {
        return;
    };
    let Some(kit) = world.get_mut::<ClassKit>(player_id) else {
        return;
    };
    kit.known_abilities = known_abilities_at_level(class, level)
        .into_iter()
        .map(str::to_string)
        .collect();
    events.push(SimEvent::Toast {
        message: format!("You are trained through level {level}."),
    });
}

fn bind_hearth(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    events: &mut Vec<SimEvent>,
) {
    if world.get::<Identity>(target_id).map(|i| i.kind) != Some(EntityKind::Npc) {
        return;
    }
    let template_id = world
        .get::<Identity>(target_id)
        .and_then(|i| i.template_id.clone())
        .unwrap_or_default();
    let Some(ndef) = npc(&template_id) else {
        return;
    };
    if !ndef.is_innkeeper() {
        return;
    }

    let Some((x, z)) = world.get::<Transform>(player_id).map(|t| (t.x, t.z)) else {
        return;
    };
    let zone_id = world
        .get::<Identity>(player_id)
        .map(|i| i.zone_id.clone())
        .unwrap_or_else(|| "eastbrook".into());
    let Some(hearth) = world.get_mut::<Hearth>(player_id) else {
        return;
    };
    hearth.zone_id = zone_id;
    hearth.x = x;
    hearth.z = z;
    events.push(SimEvent::Toast {
        message: "Hearthbound.".into(),
    });
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
        if slot >= bags.inventory.len() {
            return;
        }
        let Some(removed_stack) = bags.inventory[slot].take() else {
            return;
        };
        let durability = removed_stack
            .durability
            .or_else(|| EquipmentWear::max_for_item(&removed_stack.item_id));
        let previous = equipment_slot_mut(&mut bags.equipment, equip_slot)
            .replace(removed_stack.item_id.clone());
        let previous_wear = std::mem::replace(
            equipment_wear_slot_mut(&mut bags.equipment_wear, equip_slot),
            durability,
        );
        if let Some(prev) = previous {
            bags.inventory[slot] = Some(stack_with_durability(&prev, 1, previous_wear));
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
    let mut restored = false;
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        let Some(item_id) = equipment_slot_mut(&mut bags.equipment, equip_slot).take() else {
            return;
        };
        let wear = equipment_wear_slot_mut(&mut bags.equipment_wear, equip_slot).take();
        if let Some(empty) = bags.inventory.iter_mut().find(|slot| slot.is_none()) {
            *empty = Some(stack_with_durability(&item_id, 1, wear));
            restored = true;
        } else {
            *equipment_slot_mut(&mut bags.equipment, equip_slot) = Some(item_id.clone());
            *equipment_wear_slot_mut(&mut bags.equipment_wear, equip_slot) = wear;
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
            stun: false,
            move_mult: 1.0,
            absorb: 0.0,
            breaks_on_damage: false,
            damage_mult: 1.0,
            thorns: 0.0,
            armor_flat: 0.0,
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

fn opens_npc_session(def: &NpcDef) -> bool {
    def.is_vendor()
        || def.can_repair()
        || def.is_profession_trainer()
        || def.is_class_trainer()
        || def.is_innkeeper()
}

fn service_name(service: NpcService) -> &'static str {
    match service {
        NpcService::Vendor => "vendor",
        NpcService::Repair => "repair",
        NpcService::ProfessionTrainer => "profession_trainer",
        NpcService::ClassTrainer => "class_trainer",
        NpcService::Innkeeper => "innkeeper",
        NpcService::QuestGiver => "quest_giver",
    }
}

fn stock_snapshot(def: &NpcDef) -> Vec<VendorOfferSnapshot> {
    def.vendor_stock
        .iter()
        .filter_map(|o| {
            let price = item(o.item_id)?.vendor_buy;
            Some(VendorOfferSnapshot {
                item_id: o.item_id.to_string(),
                count: o.count,
                price,
            })
        })
        .collect()
}

pub fn vendor_snapshot(world: &World, player_id: EntityId) -> Option<woc_protocol::VendorSnapshot> {
    let npc_id = world.get::<Bags>(player_id)?.open_vendor_npc?;
    let npc_e = world.get::<Identity>(npc_id)?;
    let template = npc_e.template_id.as_deref()?;
    let def = npc(template)?;
    if !def.is_vendor() {
        return None;
    }
    let stock = stock_snapshot(def);
    Some(woc_protocol::VendorSnapshot {
        npc_id,
        npc_name: npc_e.name.clone(),
        stock,
    })
}

pub fn npc_session_snapshot(world: &World, player_id: EntityId) -> Option<NpcSessionSnapshot> {
    let bags = world.get::<Bags>(player_id)?;
    let npc_id = bags.open_vendor_npc?;
    let npc_e = world.get::<Identity>(npc_id)?;
    let template = npc_e.template_id.as_deref()?;
    let def = npc(template)?;
    let buyback = bags
        .buyback
        .iter()
        .enumerate()
        .map(|(slot, entry)| BuybackSnapshot {
            slot: slot as u8,
            item_id: entry.item_id.clone(),
            count: entry.count,
            price: entry.copper,
        })
        .collect();
    Some(NpcSessionSnapshot {
        npc_id,
        npc_name: npc_e.name.clone(),
        greeting: def.greeting.to_string(),
        services: def
            .services
            .iter()
            .copied()
            .map(service_name)
            .map(str::to_string)
            .collect(),
        stock: stock_snapshot(def),
        train_professions: def.trains.iter().map(|id| id.to_string()).collect(),
        can_repair: def.can_repair(),
        repair_cost: repair_cost(world, player_id),
        can_bind: def.is_innkeeper(),
        buyback,
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
            0,
            &mut events,
        );
        assert!(world.get::<Health>(1).unwrap().hp > 5.0);
    }
}
