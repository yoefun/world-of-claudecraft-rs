//! Profession training, gathering, and crafting simulation hooks.

use crate::ecs::components::{Bags, ClassKit, Health, Identity, Progress, Transform};
use crate::ecs::World;
use crate::inventory::{count_item, grant_into, remove_item};
use crate::quests::on_inventory_changed;
use crate::types::INTERACT_RANGE;
use woc_content::{gather_node, profession, recipe};
use woc_protocol::{EntityId, InteractAction, SimEvent};

pub type ProfessionResult = Result<(), &'static str>;

/// Route profession-related protocol actions.
///
/// Returns `true` when the action belongs to this module, including when the
/// requested operation is rejected. The sim coordinator can use this as a
/// leaf hook without duplicating action matching.
pub fn handle_interact(
    world: &mut World,
    player_id: EntityId,
    action: &InteractAction,
    events: &mut Vec<SimEvent>,
) -> bool {
    let result = match action {
        InteractAction::TrainProfession { id } => {
            train_profession(world, player_id, id, events)
        }
        InteractAction::Gather { node_id } => gather_from_entity(world, player_id, *node_id, events),
        InteractAction::Craft { recipe_id } => craft(world, player_id, recipe_id, events),
        _ => return false,
    };

    if let Err(message) = result {
        events.push(SimEvent::Toast {
            message: message.into(),
        });
    }
    true
}

/// Learn a content-defined profession at skill 1.
///
/// Training is idempotent and never lowers an existing skill rank.
pub fn train_profession(
    world: &mut World,
    player_id: EntityId,
    profession_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let definition = profession(profession_id).ok_or("unknown profession")?;
    let skill = world
        .get_mut::<Progress>(player_id)
        .ok_or("player not found")?
        .professions
        .entry(definition.id.to_string())
        .or_insert(1);
    *skill = (*skill).max(1);
    events.push(SimEvent::Toast {
        message: format!("Learned {}.", definition.name),
    });
    Ok(())
}

/// Gather a node directly by its content id.
///
/// This helper is useful for deterministic tests and server-side systems that
/// have already resolved a world node. World interaction should use
/// [`gather_from_entity`] so position and zone checks are applied.
pub fn gather_content(
    world: &mut World,
    player_id: EntityId,
    node_content_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let node = gather_node(node_content_id).ok_or("unknown gather node")?;
    let zone_id = world
        .get::<Identity>(player_id)
        .map(|i| i.zone_id.as_str())
        .unwrap_or("");
    if zone_id != node.zone_id {
        return Err("gather node is in another zone");
    }
    require_skill(world, player_id, node.profession_id, node.skill_req)?;

    let mut inventory = world
        .get::<Bags>(player_id)
        .map(|b| b.inventory.clone())
        .ok_or("player not found")?;
    if !grant_into(&mut inventory, node.item_id, node.count) {
        return Err("inventory full");
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    bump_skill(world, player_id, node.profession_id);

    events.push(SimEvent::ItemGained {
        player: player_id,
        item_id: node.item_id.into(),
        count: node.count,
    });
    events.push(SimEvent::Gathered {
        player: player_id,
        node_id: node.id.into(),
        item_id: node.item_id.into(),
        count: node.count,
    });
    on_inventory_changed(world, player_id, events);
    Ok(())
}

/// Resolve a gather-node entity through its `template_id`, then harvest it.
pub fn gather_from_entity(
    world: &mut World,
    player_id: EntityId,
    node_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    if world.get::<ClassKit>(player_id).is_none() {
        return Err("player not found");
    }
    let node = world.get::<Identity>(node_id).ok_or("gather node not found")?;
    let node_content_id = node
        .template_id
        .clone()
        .ok_or("gather node has no template")?;
    if gather_node(&node_content_id).is_none() {
        return Err("entity is not a gather node");
    }

    let (px, pz) = world
        .get::<Transform>(player_id)
        .map(|t| (t.x, t.z))
        .ok_or("player not found")?;
    let (nx, nz) = world
        .get::<Transform>(node_id)
        .map(|t| (t.x, t.z))
        .ok_or("gather node not found")?;
    let dx = px - nx;
    let dz = pz - nz;
    if (dx * dx + dz * dz).sqrt() > INTERACT_RANGE {
        return Err("gather node is too far away");
    }

    gather_content(world, player_id, &node_content_id, events)
}

/// Craft one copy of a recipe, atomically consuming reagents and granting its
/// product.
pub fn craft(
    world: &mut World,
    player_id: EntityId,
    recipe_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(world, player_id)?;
    let definition = recipe(recipe_id).ok_or("unknown recipe")?;
    require_skill(world, player_id, definition.profession_id, definition.skill_req)?;
    let inventory = world
        .get::<Bags>(player_id)
        .map(|b| &b.inventory)
        .ok_or("player not found")?;
    if definition
        .reagents
        .iter()
        .any(|reagent| count_item(inventory, reagent.item_id) < reagent.count)
    {
        return Err("missing recipe reagents");
    }

    let mut inventory = inventory.clone();
    for reagent in definition.reagents {
        if !remove_item(&mut inventory, reagent.item_id, reagent.count) {
            return Err("missing recipe reagents");
        }
    }
    if !grant_into(
        &mut inventory,
        definition.product_item_id,
        definition.product_count,
    ) {
        return Err("inventory full");
    }
    if let Some(bags) = world.get_mut::<Bags>(player_id) {
        bags.inventory = inventory;
    }
    bump_skill(world, player_id, definition.profession_id);

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
    on_inventory_changed(world, player_id, events);
    Ok(())
}

fn ensure_active_player(world: &World, player_id: EntityId) -> ProfessionResult {
    if world.get::<ClassKit>(player_id).is_none() {
        return Err("entity is not a player");
    }
    let alive = world
        .get::<Health>(player_id)
        .map(|h| h.alive)
        .unwrap_or(false);
    if !alive {
        return Err("dead players cannot use professions");
    }
    Ok(())
}

fn require_skill(world: &World, player_id: EntityId, profession_id: &str, required: u32) -> ProfessionResult {
    let skill = world
        .get::<Progress>(player_id)
        .and_then(|p| p.professions.get(profession_id).copied())
        .unwrap_or(0);
    if skill < required {
        return Err("profession skill too low");
    }
    Ok(())
}

fn bump_skill(world: &mut World, player_id: EntityId, profession_id: &str) {
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
    *skill = skill.saturating_add(1).min(definition.max_skill);
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Bags, Progress};
    use crate::inventory::count_item;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, SimEvent};

    #[test]
    fn train_gather_and_craft_minor_healing_salve() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let mut events = Vec::new();
        train_profession(&mut world, 1, "herbalism", &mut events).unwrap();
        train_profession(&mut world, 1, "alchemy", &mut events).unwrap();
        assert_eq!(
            world.get::<Progress>(1).unwrap().professions.get("herbalism"),
            Some(&1)
        );
        gather_content(&mut world, 1, "eastbrook_meadow_silverleaf", &mut events).unwrap();
        gather_content(&mut world, 1, "eastbrook_meadow_silverleaf", &mut events).unwrap();
        gather_content(&mut world, 1, "eastbrook_brook_peacebloom", &mut events).unwrap();
        assert_eq!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "silverleaf"),
            2
        );
        craft(&mut world, 1, "minor_healing_salve", &mut events).unwrap();
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
    fn interact_gather_resolves_node_entity_template() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let node = woc_content::gather_node("eastbrook_meadow_silverleaf").unwrap();
        crate::ecs::spawn::create_gather_node(&mut world, 2, node);
        let mut events = Vec::new();
        assert!(handle_interact(
            &mut world,
            1,
            &InteractAction::TrainProfession {
                id: "herbalism".into(),
            },
            &mut events,
        ));
        assert!(handle_interact(
            &mut world,
            1,
            &InteractAction::Gather { node_id: 2 },
            &mut events,
        ));
        assert_eq!(
            count_item(&world.get::<Bags>(1).unwrap().inventory, "silverleaf"),
            1
        );
    }
}
