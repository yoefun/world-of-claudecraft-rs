//! Profession training, gathering, crafting, skinning, and enchanting.

mod duration;
mod skill;

use crate::ecs::components::{
    dist2d, Bags, ClassKit, Combat, GatherNodeState, Health, Identity, InvStack, ProfessionCast,
    ProfessionCastKind, Progress, Skinnable, Transform,
};
use crate::ecs::World;
use crate::inventory::{count_item, grant_into, remove_item};
use crate::quests::on_inventory_changed;
use crate::rng::Rng;
use duration::{
    craft_cast_seconds, enchant_family_seconds, gather_cast_seconds, ticks_from_seconds,
};
use skill::{gain_skill, masterwork_proc_chance, tier_for_skill};
use woc_content::{
    base_of, craft_fee, disenchant_yield, fine_substitute_for, gather_node, in_station_range, item,
    npc, profession, profession_enchant, recipe, GatherNodeDef, ItemKind, RecipeDef,
    CRAFT_BATCH_MAX,
};
use woc_protocol::{EntityId, EntityKind, InteractAction, ProfessionDeny, SimEvent, TICK_RATE};

pub type ProfessionResult = Result<(), ProfessionDeny>;

pub const HARVEST_RANGE: f32 = 5.0;

/// Route profession-related protocol actions.
///
/// Returns `true` when the action belongs to this module, including when the
/// requested operation is rejected.
pub fn handle_interact(
    world: &mut World,
    player_id: EntityId,
    target_id: EntityId,
    action: &InteractAction,
    tick: u64,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> bool {
    let result = match action {
        InteractAction::TrainProfession { id } => {
            try_train_at_npc(world, player_id, target_id, id, events)
        }
        InteractAction::Gather { node_id } => {
            start_gather_cast(world, player_id, *node_id, tick, events)
        }
        InteractAction::Craft { recipe_id } => {
            start_craft_cast(world, player_id, recipe_id, tick, events)
        }
        InteractAction::Skin { corpse_id } => {
            start_skin_cast(world, player_id, *corpse_id, tick, events)
        }
        InteractAction::Disenchant { bag_slot } => {
            start_disenchant_cast(world, player_id, *bag_slot, tick, events)
        }
        InteractAction::ApplyEnchant {
            bag_slot,
            enchant_id,
            confirm,
        } => start_apply_enchant_cast(
            world, player_id, *bag_slot, enchant_id, *confirm, tick, events,
        ),
        _ => return false,
    };

    if let Err(reason) = result {
        events.push(SimEvent::ProfessionDenied {
            player: player_id,
            reason,
        });
    }
    let _ = rng;
    true
}

pub fn tick_profession_casts(
    world: &mut World,
    tick: u64,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) {
    let ids: Vec<EntityId> = world.ids::<ProfessionCast>();
    for player_id in ids {
        let Some(cast) = world.get::<ProfessionCast>(player_id) else {
            continue;
        };
        if tick < cast.complete_tick {
            continue;
        }
        let kind = world.remove::<ProfessionCast>(player_id).map(|c| c.kind);
        let Some(kind) = kind else {
            continue;
        };
        let result = match kind {
            ProfessionCastKind::Gather { node_id } => {
                gather_from_entity(world, player_id, node_id, tick, rng, events)
            }
            ProfessionCastKind::Skin { corpse_id } => {
                complete_skin(world, player_id, corpse_id, rng, events)
            }
            ProfessionCastKind::Craft {
                recipe_id,
                remaining,
            } => complete_craft(world, player_id, &recipe_id, remaining, rng, events),
            ProfessionCastKind::Disenchant { bag_slot } => {
                complete_disenchant(world, player_id, bag_slot, events)
            }
            ProfessionCastKind::ApplyEnchant {
                bag_slot,
                enchant_id,
                confirm,
            } => complete_apply_enchant(world, player_id, bag_slot, &enchant_id, confirm, events),
        };
        if let Err(reason) = result {
            events.push(SimEvent::ProfessionDenied {
                player: player_id,
                reason,
            });
        }
    }
}

fn try_train_at_npc(
    world: &mut World,
    player_id: EntityId,
    npc_id: EntityId,
    profession_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let template_id = world
        .get::<Identity>(npc_id)
        .filter(|identity| identity.kind == EntityKind::Npc)
        .and_then(|identity| identity.template_id.clone())
        .ok_or(ProfessionDeny::UnknownProfession)?;
    if dist2d(world, player_id, npc_id)
        .map(|distance| distance > crate::types::INTERACT_RANGE)
        .unwrap_or(true)
    {
        return Err(ProfessionDeny::OutOfRange);
    }
    if !npc(&template_id).is_some_and(|definition| definition.trains_profession(profession_id)) {
        return Err(ProfessionDeny::UnknownProfession);
    }
    train_profession(world, player_id, profession_id, events)
}

pub fn train_profession(
    world: &mut World,
    player_id: EntityId,
    profession_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let definition = profession(profession_id).ok_or(ProfessionDeny::UnknownProfession)?;
    let skill = world
        .get_mut::<Progress>(player_id)
        .ok_or(ProfessionDeny::NotPlayer)?
        .professions
        .entry(definition.id.to_string())
        .or_insert(1);
    *skill = (*skill).max(1);
    events.push(SimEvent::Toast {
        message: format!("Learned {}.", definition.name),
    });
    Ok(())
}

pub fn gather_content(
    world: &mut World,
    player_id: EntityId,
    node_content_id: &str,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let node = gather_node(node_content_id).ok_or(ProfessionDeny::UnknownNode)?;
    let zone_id = world
        .get::<Identity>(player_id)
        .map(|i| i.zone_id.as_str())
        .unwrap_or("");
    if zone_id != node.zone_id {
        return Err(ProfessionDeny::UnknownNode);
    }
    complete_gather_node(world, player_id, node, None, 0, rng, events)
}

pub fn gather_from_entity(
    world: &mut World,
    player_id: EntityId,
    node_id: EntityId,
    tick: u64,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let node = world
        .get::<Identity>(node_id)
        .ok_or(ProfessionDeny::UnknownNode)?;
    let node_content_id = node
        .template_id
        .clone()
        .ok_or(ProfessionDeny::UnknownNode)?;
    let def = gather_node(&node_content_id).ok_or(ProfessionDeny::UnknownNode)?;
    range_check(world, player_id, node_id)?;
    complete_gather_node(world, player_id, def, Some(node_id), tick, rng, events)
}

fn start_gather_cast(
    world: &mut World,
    player_id: EntityId,
    node_id: EntityId,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let _ = events;
    evaluate_gather_entity(world, player_id, node_id, tick)?;
    let profession_id = world
        .get::<Identity>(node_id)
        .and_then(|i| i.template_id.clone())
        .and_then(|id| gather_node(&id).map(|n| n.profession_id))
        .unwrap_or("herbalism");
    let skill = profession_skill(world, player_id, profession_id);
    let duration = ticks_from_seconds(gather_cast_seconds(0, tier_for_skill(skill)));
    begin_cast(
        world,
        player_id,
        ProfessionCastKind::Gather { node_id },
        tick,
        duration,
    )
}

fn evaluate_gather_entity(
    world: &World,
    player_id: EntityId,
    node_id: EntityId,
    tick: u64,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    let identity = world
        .get::<Identity>(node_id)
        .ok_or(ProfessionDeny::UnknownNode)?;
    let content_id = identity
        .template_id
        .as_deref()
        .ok_or(ProfessionDeny::UnknownNode)?;
    let node = gather_node(content_id).ok_or(ProfessionDeny::UnknownNode)?;
    range_check(world, player_id, node_id)?;
    let ready_tick = world
        .get::<GatherNodeState>(node_id)
        .map(|s| s.ready_tick)
        .unwrap_or(0);
    if tick < ready_tick {
        return Err(ProfessionDeny::NodeNotReady);
    }
    require_tool(world, player_id, node.tool_item_id)?;
    Ok(())
}

fn complete_gather_node(
    world: &mut World,
    player_id: EntityId,
    node: &GatherNodeDef,
    node_entity: Option<EntityId>,
    tick: u64,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    require_tool(world, player_id, node.tool_item_id)?;

    let rare = rng.chance(15);
    let double = rng.chance(20);
    let item_id = if rare {
        node.fine_item_id.unwrap_or(node.item_id)
    } else {
        node.item_id
    };
    let count = if rare {
        5
    } else if double {
        2
    } else {
        node.count.max(1)
    };

    let mut stacks: Vec<(&str, u32)> = vec![(item_id, count)];
    if double && !rare {
        if let Some(bonus) = node.bonus_item_id {
            stacks.push((bonus, 1));
        }
    }

    let mut inventory = world
        .get::<Bags>(player_id)
        .map(|b| b.inventory.clone())
        .ok_or(ProfessionDeny::NotPlayer)?;
    for (id, n) in &stacks {
        if !grant_into(&mut inventory, id, *n) {
            return Err(ProfessionDeny::InventoryFull);
        }
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    bump_skill(world, player_id, node.profession_id, node.skill_req);

    if let Some(node_id) = node_entity {
        if let Some(state) = world.get_mut::<GatherNodeState>(node_id) {
            state.ready_tick =
                tick.saturating_add(u64::from(node.respawn_seconds) * u64::from(TICK_RATE));
        }
    }

    for (id, n) in &stacks {
        events.push(SimEvent::ItemGained {
            player: player_id,
            item_id: (*id).into(),
            count: *n,
        });
    }
    events.push(SimEvent::Gathered {
        player: player_id,
        node_id: node.id.into(),
        item_id: item_id.into(),
        count,
    });
    on_inventory_changed(world, player_id, events);
    Ok(())
}

pub fn craft(
    world: &mut World,
    player_id: EntityId,
    recipe_id: &str,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    complete_craft(world, player_id, recipe_id, 1, rng, events)
}

fn start_craft_cast(
    world: &mut World,
    player_id: EntityId,
    recipe_id: &str,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let _ = events;
    evaluate_craft(world, player_id, recipe_id, 1)?;
    let def = recipe(recipe_id).ok_or(ProfessionDeny::UnknownRecipe)?;
    let duration = ticks_from_seconds(craft_cast_seconds(def.skill_req));
    begin_cast(
        world,
        player_id,
        ProfessionCastKind::Craft {
            recipe_id: recipe_id.to_string(),
            remaining: 1,
        },
        tick,
        duration,
    )
}

fn evaluate_craft(
    world: &World,
    player_id: EntityId,
    recipe_id: &str,
    count: u32,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    if count == 0 || count > CRAFT_BATCH_MAX {
        return Err(ProfessionDeny::InvalidCount);
    }
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    let definition = recipe(recipe_id).ok_or(ProfessionDeny::UnknownRecipe)?;
    if let Some(station_id) = definition.station {
        let (x, z) = player_xz(world, player_id)?;
        if !in_station_range(x, z, station_id) {
            return Err(ProfessionDeny::StationRequired);
        }
    }
    let inventory = world
        .get::<Bags>(player_id)
        .map(|b| &b.inventory)
        .ok_or(ProfessionDeny::NotPlayer)?;
    if definition.reagents.iter().any(|reagent| {
        available_count(inventory, reagent.item_id) < reagent.count.saturating_mul(count)
    }) {
        return Err(ProfessionDeny::MissingReagents);
    }
    let fee = craft_fee(definition).saturating_mul(count);
    let copper = world
        .get::<Progress>(player_id)
        .map(|p| p.copper)
        .unwrap_or(0);
    if copper < fee {
        return Err(ProfessionDeny::InsufficientGold);
    }
    let mut trial = inventory.to_vec();
    for _ in 0..count {
        if !can_fit_craft(&mut trial, definition) {
            return Err(ProfessionDeny::InventoryFull);
        }
    }
    Ok(())
}

fn complete_craft(
    world: &mut World,
    player_id: EntityId,
    recipe_id: &str,
    count: u16,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    evaluate_craft(world, player_id, recipe_id, u32::from(count.max(1)))?;
    let definition = recipe(recipe_id).ok_or(ProfessionDeny::UnknownRecipe)?;
    let fee = craft_fee(definition);
    let mut crafted = 0u16;

    for _ in 0..count.max(1) {
        let inventory = world
            .get::<Bags>(player_id)
            .map(|b| b.inventory.clone())
            .ok_or(ProfessionDeny::NotPlayer)?;
        if definition
            .reagents
            .iter()
            .any(|r| available_count(&inventory, r.item_id) < r.count)
        {
            break;
        }
        let copper = world
            .get::<Progress>(player_id)
            .map(|p| p.copper)
            .unwrap_or(0);
        if copper < fee {
            break;
        }
        let mut trial = inventory.clone();
        if !can_fit_craft(&mut trial, definition) {
            break;
        }
        let mut inv = inventory;
        if !remove_reagents(&mut inv, definition) {
            break;
        }
        if !grant_into(
            &mut inv,
            definition.product_item_id,
            definition.product_count,
        ) {
            return Err(ProfessionDeny::InventoryFull);
        }
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            bags.inventory = inv;
        }
        if let Some(progress) = world.get_mut::<Progress>(player_id) {
            progress.copper = progress.copper.saturating_sub(fee);
        }
        let skill = profession_skill(world, player_id, definition.profession_id);
        if rng.chance(masterwork_proc_chance(skill, definition.skill_req)) {
            if let Some(progress) = world.get_mut::<Progress>(player_id) {
                progress.last_masterwork = Some(definition.id.to_string());
            }
        }
        bump_skill(
            world,
            player_id,
            definition.profession_id,
            definition.skill_req,
        );
        crafted += 1;
        for reagent in definition.reagents {
            events.push(SimEvent::ItemLost {
                player: player_id,
                item_id: reagent.item_id.into(),
                count: reagent.count,
            });
        }
        events.push(SimEvent::ItemGained {
            player: player_id,
            item_id: definition.product_item_id.into(),
            count: definition.product_count,
        });
        events.push(SimEvent::Crafted {
            player: player_id,
            recipe_id: definition.id.into(),
            item_id: definition.product_item_id.into(),
            count: definition.product_count,
        });
    }

    if crafted == 0 {
        return Err(ProfessionDeny::MissingReagents);
    }
    on_inventory_changed(world, player_id, events);
    Ok(())
}

fn start_skin_cast(
    world: &mut World,
    player_id: EntityId,
    corpse_id: EntityId,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let _ = events;
    evaluate_skin(world, player_id, corpse_id)?;
    let duration = ticks_from_seconds(gather_cast_seconds(0, 0));
    begin_cast(
        world,
        player_id,
        ProfessionCastKind::Skin { corpse_id },
        tick,
        duration,
    )
}

fn evaluate_skin(world: &World, player_id: EntityId, corpse_id: EntityId) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    range_check(world, player_id, corpse_id)?;
    require_tool(world, player_id, "skinning_knife").map_err(|_| ProfessionDeny::MissingKnife)?;
    let skin = world
        .get::<Skinnable>(corpse_id)
        .ok_or(ProfessionDeny::NothingToSkin)?;
    if skin.skinned {
        return Err(ProfessionDeny::AlreadySkinned);
    }
    Ok(())
}

fn complete_skin(
    world: &mut World,
    player_id: EntityId,
    corpse_id: EntityId,
    rng: &mut Rng,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    evaluate_skin(world, player_id, corpse_id)?;
    let rare = rng.chance(15);
    let item_id = if rare {
        "fine_light_leather"
    } else {
        "light_leather"
    };
    let mut inventory = world
        .get::<Bags>(player_id)
        .map(|b| b.inventory.clone())
        .ok_or(ProfessionDeny::NotPlayer)?;
    if !grant_into(&mut inventory, item_id, 1) {
        return Err(ProfessionDeny::InventoryFull);
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    if let Some(skin) = world.get_mut::<Skinnable>(corpse_id) {
        skin.skinned = true;
    }
    bump_skill(world, player_id, "skinning", 0);
    events.push(SimEvent::ItemGained {
        player: player_id,
        item_id: item_id.into(),
        count: 1,
    });
    events.push(SimEvent::Skinned {
        player: player_id,
        corpse_id,
        item_id: item_id.into(),
        count: 1,
    });
    on_inventory_changed(world, player_id, events);
    Ok(())
}

fn start_disenchant_cast(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let _ = events;
    evaluate_disenchant(world, player_id, bag_slot)?;
    let duration = ticks_from_seconds(enchant_family_seconds());
    begin_cast(
        world,
        player_id,
        ProfessionCastKind::Disenchant { bag_slot },
        tick,
        duration,
    )
}

fn evaluate_disenchant(world: &World, player_id: EntityId, bag_slot: u8) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    let stack = bag_stack(world, player_id, bag_slot).ok_or(ProfessionDeny::NotInstanced)?;
    let def = item(&stack.item_id).ok_or(ProfessionDeny::NotInstanced)?;
    if !matches!(def.kind, ItemKind::Weapon | ItemKind::Armor) {
        return Err(ProfessionDeny::NotInstanced);
    }
    Ok(())
}

fn complete_disenchant(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    evaluate_disenchant(world, player_id, bag_slot)?;
    let stack = bag_stack(world, player_id, bag_slot).ok_or(ProfessionDeny::NotInstanced)?;
    let destroyed = stack.item_id.clone();
    let yields = disenchant_yield(&destroyed);
    let mut inventory = world
        .get::<Bags>(player_id)
        .map(|b| b.inventory.clone())
        .ok_or(ProfessionDeny::NotPlayer)?;
    let idx = bag_slot as usize;
    if idx >= inventory.len() || inventory[idx].is_none() {
        return Err(ProfessionDeny::NotInstanced);
    }
    inventory[idx] = None;
    for reagent in yields {
        if !grant_into(&mut inventory, reagent.item_id, reagent.count) {
            return Err(ProfessionDeny::InventoryFull);
        }
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    bump_skill(world, player_id, "enchanting", 0);
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id: destroyed.clone(),
        count: 1,
    });
    for reagent in yields {
        events.push(SimEvent::ItemGained {
            player: player_id,
            item_id: reagent.item_id.into(),
            count: reagent.count,
        });
    }
    events.push(SimEvent::Disenchanted {
        player: player_id,
        item_id: destroyed,
    });
    on_inventory_changed(world, player_id, events);
    Ok(())
}

fn start_apply_enchant_cast(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    enchant_id: &str,
    confirm: bool,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let _ = events;
    evaluate_apply_enchant(world, player_id, bag_slot, enchant_id, confirm)?;
    let duration = ticks_from_seconds(enchant_family_seconds());
    begin_cast(
        world,
        player_id,
        ProfessionCastKind::ApplyEnchant {
            bag_slot,
            enchant_id: enchant_id.to_string(),
            confirm,
        },
        tick,
        duration,
    )
}

fn evaluate_apply_enchant(
    world: &World,
    player_id: EntityId,
    bag_slot: u8,
    enchant_id: &str,
    confirm: bool,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    let enchant_def = profession_enchant(enchant_id).ok_or(ProfessionDeny::UnknownEnchant)?;
    let stack = bag_stack(world, player_id, bag_slot).ok_or(ProfessionDeny::NotInstanced)?;
    let item_def = item(&stack.item_id).ok_or(ProfessionDeny::NotInstanced)?;
    let Some(slot) = item_def.equip_slot else {
        return Err(ProfessionDeny::WrongSlot);
    };
    if slot != enchant_def.slot {
        return Err(ProfessionDeny::WrongSlot);
    }
    if let Some(current) = stack.enchant_id.as_deref() {
        if current == enchant_id {
            return Err(ProfessionDeny::SameEnchant);
        }
        if !confirm {
            return Err(ProfessionDeny::AlreadyEnchanted);
        }
    }
    let inventory = world
        .get::<Bags>(player_id)
        .map(|b| &b.inventory)
        .ok_or(ProfessionDeny::NotPlayer)?;
    if enchant_def
        .reagents
        .iter()
        .any(|r| count_item(inventory, r.item_id) < r.count)
    {
        return Err(ProfessionDeny::MissingReagents);
    }
    Ok(())
}

fn complete_apply_enchant(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    enchant_id: &str,
    confirm: bool,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    evaluate_apply_enchant(world, player_id, bag_slot, enchant_id, confirm)?;
    let enchant_def = profession_enchant(enchant_id).ok_or(ProfessionDeny::UnknownEnchant)?;
    let mut inventory = world
        .get::<Bags>(player_id)
        .map(|b| b.inventory.clone())
        .ok_or(ProfessionDeny::NotPlayer)?;
    for reagent in enchant_def.reagents {
        if !remove_item(&mut inventory, reagent.item_id, reagent.count) {
            return Err(ProfessionDeny::MissingReagents);
        }
    }
    let idx = bag_slot as usize;
    let item_id = inventory
        .get_mut(idx)
        .and_then(|s| s.as_mut())
        .ok_or(ProfessionDeny::NotInstanced)?;
    item_id.enchant_id = Some(enchant_id.to_string());
    let granted_item = item_id.item_id.clone();
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    bump_skill(world, player_id, "enchanting", 0);
    events.push(SimEvent::EnchantApplied {
        player: player_id,
        item_id: granted_item,
        enchant_id: enchant_id.into(),
    });
    on_inventory_changed(world, player_id, events);
    Ok(())
}

fn begin_cast(
    world: &mut World,
    player_id: EntityId,
    kind: ProfessionCastKind,
    tick: u64,
    duration_ticks: u32,
) -> ProfessionResult {
    if is_busy(world, player_id) {
        return Err(ProfessionDeny::Busy);
    }
    world.insert(
        player_id,
        ProfessionCast {
            kind,
            complete_tick: tick.saturating_add(u64::from(duration_ticks.max(1))),
        },
    );
    Ok(())
}

fn available_count(inv: &[Option<InvStack>], item_id: &str) -> u32 {
    let exact = count_item(inv, item_id);
    if base_of(item_id) != item_id {
        return exact;
    }
    exact
        + fine_substitute_for(item_id)
            .map(|fine| count_item(inv, fine))
            .unwrap_or(0)
}

fn remove_reagent(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    let mut remaining = count;
    let from_exact = count_item(inv, item_id).min(remaining);
    if from_exact > 0 && !remove_item(inv, item_id, from_exact) {
        return false;
    }
    remaining = remaining.saturating_sub(from_exact);
    if remaining == 0 {
        return true;
    }
    if base_of(item_id) == item_id {
        if let Some(fine) = fine_substitute_for(item_id) {
            return remove_item(inv, fine, remaining);
        }
    }
    false
}

fn remove_reagents(inv: &mut [Option<InvStack>], recipe: &RecipeDef) -> bool {
    recipe
        .reagents
        .iter()
        .all(|r| remove_reagent(inv, r.item_id, r.count))
}

fn can_fit_craft(trial: &mut [Option<InvStack>], recipe: &RecipeDef) -> bool {
    if !remove_reagents(trial, recipe) {
        return false;
    }
    grant_into(trial, recipe.product_item_id, recipe.product_count)
}

fn require_tool(world: &World, player_id: EntityId, tool_item_id: &str) -> ProfessionResult {
    let inventory = world
        .get::<Bags>(player_id)
        .map(|b| &b.inventory)
        .ok_or(ProfessionDeny::NotPlayer)?;
    if count_item(inventory, tool_item_id) == 0 {
        return Err(ProfessionDeny::MissingTool);
    }
    Ok(())
}

fn bag_stack(world: &World, player_id: EntityId, bag_slot: u8) -> Option<InvStack> {
    world
        .get::<Bags>(player_id)?
        .inventory
        .get(bag_slot as usize)?
        .clone()
}

fn range_check(world: &World, player_id: EntityId, target_id: EntityId) -> ProfessionResult {
    if dist2d(world, player_id, target_id)
        .map(|distance| distance > HARVEST_RANGE)
        .unwrap_or(true)
    {
        return Err(ProfessionDeny::OutOfRange);
    }
    Ok(())
}

fn player_xz(world: &World, player_id: EntityId) -> Result<(f32, f32), ProfessionDeny> {
    world
        .get::<Transform>(player_id)
        .map(|t| (t.x, t.z))
        .ok_or(ProfessionDeny::NotPlayer)
}

fn is_busy(world: &World, player_id: EntityId) -> bool {
    world.get::<ProfessionCast>(player_id).is_some()
        || world
            .get::<Combat>(player_id)
            .is_some_and(|c| c.cast.is_some())
}

fn ensure_active_player(world: &World, player_id: EntityId) -> ProfessionResult {
    if world.get::<ClassKit>(player_id).is_none() {
        return Err(ProfessionDeny::NotPlayer);
    }
    let alive = world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false);
    if !alive {
        return Err(ProfessionDeny::Dead);
    }
    Ok(())
}

fn profession_skill(world: &World, player_id: EntityId, profession_id: &str) -> u32 {
    world
        .get::<Progress>(player_id)
        .and_then(|p| p.professions.get(profession_id).copied())
        .unwrap_or(0)
}

fn bump_skill(world: &mut World, player_id: EntityId, profession_id: &str, skill_req: u32) {
    let Some(definition) = profession(profession_id) else {
        return;
    };
    let Some(progress) = world.get_mut::<Progress>(player_id) else {
        return;
    };
    let skill = progress
        .professions
        .entry(profession_id.to_string())
        .or_default();
    *skill += gain_skill(*skill, skill_req, definition.max_skill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Progress};
    use crate::inventory::count_item;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, ProfessionDeny, SimEvent};

    fn grant(world: &mut World, player: EntityId, item_id: &str, count: u32) {
        if let Some(bags) = world.get_mut::<Bags>(player) {
            assert!(grant_into(&mut bags.inventory, item_id, count));
        }
    }

    #[test]
    fn train_gather_and_craft_minor_healing_salve() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        grant(&mut world, 1, "copper_sickle", 1);
        let mut events = Vec::new();
        let mut rng = Rng::new(1);
        train_profession(&mut world, 1, "herbalism", &mut events).unwrap();
        train_profession(&mut world, 1, "alchemy", &mut events).unwrap();
        assert_eq!(
            world
                .get::<Progress>(1)
                .unwrap()
                .professions
                .get("herbalism"),
            Some(&1)
        );
        gather_content(
            &mut world,
            1,
            "eastbrook_meadow_silverleaf",
            &mut rng,
            &mut events,
        )
        .unwrap();
        gather_content(
            &mut world,
            1,
            "eastbrook_meadow_silverleaf",
            &mut rng,
            &mut events,
        )
        .unwrap();
        gather_content(
            &mut world,
            1,
            "eastbrook_brook_peacebloom",
            &mut rng,
            &mut events,
        )
        .unwrap();
        assert!(count_item(&world.get::<Bags>(1).unwrap().inventory, "silverleaf") >= 2);
        craft(&mut world, 1, "minor_healing_salve", &mut rng, &mut events).unwrap();
        assert_eq!(
            count_item(
                &world.get::<Bags>(1).unwrap().inventory,
                "minor_healing_salve"
            ),
            1
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::Crafted {
                recipe_id,
                item_id,
                ..
            } if recipe_id == "minor_healing_salve" && item_id == "minor_healing_salve"
        )));
    }

    #[test]
    fn gather_without_tool_is_denied() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let mut events = Vec::new();
        let mut rng = Rng::new(1);
        let err = gather_content(
            &mut world,
            1,
            "eastbrook_meadow_silverleaf",
            &mut rng,
            &mut events,
        )
        .unwrap_err();
        assert_eq!(err, ProfessionDeny::MissingTool);
    }

    #[test]
    fn train_mine_and_craft_copper_shortsword_then_equip() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Smith", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        let mut rng = Rng::new(1);
        train_profession(&mut world, 1, "mining", &mut events).unwrap();
        train_profession(&mut world, 1, "blacksmithing", &mut events).unwrap();
        if let Some(progress) = world.get_mut::<Progress>(1) {
            progress.copper = 100;
        }
        grant(&mut world, 1, "copper_ore", 6);
        grant(&mut world, 1, "smithing_flux", 2);
        craft(&mut world, 1, "smelt_copper_bar", &mut rng, &mut events).unwrap();
        craft(&mut world, 1, "smelt_copper_bar", &mut rng, &mut events).unwrap();
        craft(&mut world, 1, "smelt_copper_bar", &mut rng, &mut events).unwrap();
        craft(&mut world, 1, "copper_shortsword", &mut rng, &mut events).unwrap();
        assert_eq!(
            count_item(
                &world.get::<Bags>(1).unwrap().inventory,
                "copper_shortsword"
            ),
            1
        );
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .is_some_and(|st| st.item_id == "copper_shortsword")
            })
            .expect("sword in bag") as u8;
        crate::interaction::handle_interact(
            &mut world,
            1,
            1,
            InteractAction::Equip { bag_slot: slot },
            0,
            &mut events,
        );
        assert_eq!(
            world.get::<Bags>(1).unwrap().equipment.off_hand.as_deref(),
            Some("copper_shortsword")
        );
        assert_eq!(
            world.get::<Bags>(1).unwrap().equipment.main_hand.as_deref(),
            Some("worn_sword")
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::Equipped { item_id, .. } if item_id == "copper_shortsword"
        )));
    }

    #[test]
    fn shortsword_requires_forge() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Smith", PlayerClass::Warrior, 80.0, 80.0);
        if let Some(progress) = world.get_mut::<Progress>(1) {
            progress.copper = 100;
        }
        grant(&mut world, 1, "copper_bar", 3);
        grant(&mut world, 1, "smithing_flux", 2);
        let mut rng = Rng::new(1);
        let mut events = Vec::new();
        let err = craft(&mut world, 1, "copper_shortsword", &mut rng, &mut events).unwrap_err();
        assert_eq!(err, ProfessionDeny::StationRequired);
    }

    #[test]
    fn interact_gather_resolves_node_entity_template() {
        let mut world = World::new();
        let node = woc_content::gather_node("eastbrook_meadow_silverleaf").unwrap();
        crate::ecs::spawn::create_player(
            &mut world,
            1,
            "Herbalist",
            PlayerClass::Druid,
            node.x,
            node.z,
        );
        crate::ecs::spawn::create_gather_node(&mut world, 2, node);
        grant(&mut world, 1, "copper_sickle", 1);
        let mut events = Vec::new();
        let mut rng = Rng::new(1);
        assert!(handle_interact(
            &mut world,
            1,
            2,
            &InteractAction::Gather { node_id: 2 },
            0,
            &mut rng,
            &mut events,
        ));
        tick_profession_casts(&mut world, 50, &mut rng, &mut events);
        assert!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "silverleaf")
                + count_item(&world.get::<Bags>(1).unwrap().inventory, "fine_silverleaf")
                >= 1
        );
    }

    #[test]
    fn profession_deny_is_stable_id_not_english_toast() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let mut events = Vec::new();
        let mut rng = Rng::new(1);
        assert!(handle_interact(
            &mut world,
            1,
            1,
            &InteractAction::Craft {
                recipe_id: "nope".into(),
            },
            0,
            &mut rng,
            &mut events,
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::ProfessionDenied {
                reason: ProfessionDeny::UnknownRecipe,
                ..
            }
        )));
        assert!(!events
            .iter()
            .any(|event| matches!(event, SimEvent::Toast { .. })));
    }

    #[test]
    fn skin_loot_once() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Skinner", PlayerClass::Rogue, 0.0, 0.0);
        crate::ecs::spawn::create_loot(&mut world, 9, 0.0, 0.0, 0, None);
        crate::ecs::spawn::maybe_mark_skinnable(&mut world, 9, "young_wolf");
        grant(&mut world, 1, "skinning_knife", 1);
        let mut rng = Rng::new(1);
        let mut events = Vec::new();
        complete_skin(&mut world, 1, 9, &mut rng, &mut events).unwrap();
        assert_eq!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "light_leather")
                + count_item(
                    &world.get::<Bags>(1).unwrap().inventory,
                    "fine_light_leather"
                ),
            1
        );
        let err = complete_skin(&mut world, 1, 9, &mut rng, &mut events).unwrap_err();
        assert_eq!(err, ProfessionDeny::AlreadySkinned);
    }

    #[test]
    fn disenchant_and_apply_weapon_enchant() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Enchanter", PlayerClass::Mage, 0.0, 0.0);
        grant(&mut world, 1, "copper_shortsword", 1);
        grant(&mut world, 1, "arcane_dust", 6);
        let slot = world
            .get::<Bags>(1)
            .unwrap()
            .inventory
            .iter()
            .position(|s| {
                s.as_ref()
                    .is_some_and(|st| st.item_id == "copper_shortsword")
            })
            .unwrap() as u8;
        let mut events = Vec::new();
        complete_apply_enchant(
            &mut world,
            1,
            slot,
            "weapon_minor_might",
            false,
            &mut events,
        )
        .unwrap();
        assert_eq!(
            world.get::<Bags>(1).unwrap().inventory[slot as usize]
                .as_ref()
                .unwrap()
                .enchant_id
                .as_deref(),
            Some("weapon_minor_might")
        );
        complete_disenchant(&mut world, 1, slot, &mut events).unwrap();
        assert_eq!(
            count_item(
                &world.get::<Bags>(1).unwrap().inventory,
                "copper_shortsword"
            ),
            0
        );
        assert!(count_item(&world.get::<Bags>(1).unwrap().inventory, "arcane_dust") >= 1);
    }
}
