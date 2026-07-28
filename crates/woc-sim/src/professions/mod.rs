//! Profession training, gathering, and crafting simulation hooks.

use crate::entity::{count_item, grant_into, remove_item, Entity};
use crate::types::INTERACT_RANGE;
use woc_content::{gather_node, profession, recipe};
use woc_protocol::{EntityId, EntityKind, InteractAction, SimEvent};

pub type ProfessionResult = Result<(), &'static str>;

/// Route profession-related protocol actions.
///
/// Returns `true` when the action belongs to this module, including when the
/// requested operation is rejected. The sim coordinator can use this as a
/// leaf hook without duplicating action matching.
pub fn handle_interact(
    entities: &mut [Entity],
    player_id: EntityId,
    action: &InteractAction,
    events: &mut Vec<SimEvent>,
) -> bool {
    let result = match action {
        InteractAction::TrainProfession { id } => with_player_mut(entities, player_id, |player| {
            train_profession(player, id, events)
        }),
        InteractAction::Gather { node_id } => {
            gather_from_entity(entities, player_id, *node_id, events)
        }
        InteractAction::Craft { recipe_id } => with_player_mut(entities, player_id, |player| {
            craft(player, recipe_id, events)
        }),
        _ => return false,
    };

    if let Err(message) = result {
        events.push(SimEvent::Toast {
            message: message.into(),
        });
    }
    true
}

fn with_player_mut(
    entities: &mut [Entity],
    player_id: EntityId,
    operation: impl FnOnce(&mut Entity) -> ProfessionResult,
) -> ProfessionResult {
    let player = entities
        .iter_mut()
        .find(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
        .ok_or("player not found")?;
    operation(player)
}

/// Learn a content-defined profession at skill 1.
///
/// Training is idempotent and never lowers an existing skill rank.
pub fn train_profession(
    player: &mut Entity,
    profession_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(player)?;
    let definition = profession(profession_id).ok_or("unknown profession")?;
    let skill = player
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
    player: &mut Entity,
    node_content_id: &str,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    ensure_active_player(player)?;
    let node = gather_node(node_content_id).ok_or("unknown gather node")?;
    if player.zone_id != node.zone_id {
        return Err("gather node is in another zone");
    }
    require_skill(player, node.profession_id, node.skill_req)?;

    let mut inventory = player.inventory.clone();
    if !grant_into(&mut inventory, node.item_id, node.count) {
        return Err("inventory full");
    }
    player.inventory = inventory;
    bump_skill(player, node.profession_id);

    events.push(SimEvent::ItemGained {
        player: player.id,
        item_id: node.item_id.into(),
        count: node.count,
    });
    events.push(SimEvent::Gathered {
        player: player.id,
        node_id: node.id.into(),
        item_id: node.item_id.into(),
        count: node.count,
    });
    crate::quests::on_inventory_changed(player, events);
    Ok(())
}

/// Resolve a gather-node entity through its `template_id`, then harvest it.
pub fn gather_from_entity(
    entities: &mut [Entity],
    player_id: EntityId,
    node_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> ProfessionResult {
    let player_index = entities
        .iter()
        .position(|entity| entity.id == player_id && entity.kind == EntityKind::Player)
        .ok_or("player not found")?;
    let node = entities
        .iter()
        .find(|entity| entity.id == node_id)
        .ok_or("gather node not found")?;
    let node_content_id = node
        .template_id
        .clone()
        .ok_or("gather node has no template")?;
    if gather_node(&node_content_id).is_none() {
        return Err("entity is not a gather node");
    }

    let dx = entities[player_index].x - node.x;
    let dz = entities[player_index].z - node.z;
    if (dx * dx + dz * dz).sqrt() > INTERACT_RANGE {
        return Err("gather node is too far away");
    }

    gather_content(&mut entities[player_index], &node_content_id, events)
}

/// Craft one copy of a recipe, atomically consuming reagents and granting its
/// product.
pub fn craft(player: &mut Entity, recipe_id: &str, events: &mut Vec<SimEvent>) -> ProfessionResult {
    ensure_active_player(player)?;
    let definition = recipe(recipe_id).ok_or("unknown recipe")?;
    require_skill(player, definition.profession_id, definition.skill_req)?;
    if definition
        .reagents
        .iter()
        .any(|reagent| count_item(&player.inventory, reagent.item_id) < reagent.count)
    {
        return Err("missing recipe reagents");
    }

    let mut inventory = player.inventory.clone();
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
    player.inventory = inventory;
    bump_skill(player, definition.profession_id);

    for reagent in definition.reagents {
        events.push(SimEvent::ItemLost {
            player: player.id,
            item_id: reagent.item_id.into(),
            count: reagent.count,
        });
    }
    events.push(SimEvent::ItemGained {
        player: player.id,
        item_id: definition.product_item_id.into(),
        count: definition.product_count,
    });
    events.push(SimEvent::Crafted {
        player: player.id,
        recipe_id: definition.id.into(),
        item_id: definition.product_item_id.into(),
        count: definition.product_count,
    });
    crate::quests::on_inventory_changed(player, events);
    Ok(())
}

fn ensure_active_player(player: &Entity) -> ProfessionResult {
    if player.kind != EntityKind::Player {
        return Err("entity is not a player");
    }
    if !player.alive {
        return Err("dead players cannot use professions");
    }
    Ok(())
}

fn require_skill(player: &Entity, profession_id: &str, required: u32) -> ProfessionResult {
    let skill = player.professions.get(profession_id).copied().unwrap_or(0);
    if skill < required {
        return Err("profession skill too low");
    }
    Ok(())
}

fn bump_skill(player: &mut Entity, profession_id: &str) {
    let Some(definition) = profession(profession_id) else {
        return;
    };
    let skill = player
        .professions
        .entry(profession_id.to_string())
        .or_default();
    *skill = skill.saturating_add(1).min(definition.max_skill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{count_item, create_player, Entity};
    use woc_content::PlayerClass;
    use woc_protocol::{EntityKind, InteractAction, SimEvent};

    #[test]
    fn train_gather_and_craft_minor_healing_salve() {
        let mut player = create_player(1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let mut events = Vec::new();

        train_profession(&mut player, "herbalism", &mut events).unwrap();
        train_profession(&mut player, "alchemy", &mut events).unwrap();
        assert_eq!(player.professions.get("herbalism"), Some(&1));
        assert_eq!(player.professions.get("alchemy"), Some(&1));

        gather_content(&mut player, "eastbrook_meadow_silverleaf", &mut events).unwrap();
        gather_content(&mut player, "eastbrook_meadow_silverleaf", &mut events).unwrap();
        gather_content(&mut player, "eastbrook_brook_peacebloom", &mut events).unwrap();
        assert_eq!(count_item(&player.inventory, "silverleaf"), 2);
        assert_eq!(count_item(&player.inventory, "peacebloom"), 1);

        craft(&mut player, "minor_healing_salve", &mut events).unwrap();
        assert_eq!(count_item(&player.inventory, "silverleaf"), 0);
        assert_eq!(count_item(&player.inventory, "peacebloom"), 0);
        assert_eq!(count_item(&player.inventory, "minor_healing_salve"), 1);
        assert!(player.professions["herbalism"] > 1);
        assert!(player.professions["alchemy"] > 1);
        assert!(events.iter().any(|event| matches!(
            event,
            SimEvent::Gathered {
                node_id,
                item_id,
                ..
            } if node_id == "eastbrook_meadow_silverleaf" && item_id == "silverleaf"
        )));
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
        let player = create_player(1, "Herbalist", PlayerClass::Druid, 0.0, 0.0);
        let mut node = Entity::blank(
            2,
            EntityKind::Loot,
            "Silverleaf Patch",
            Some("eastbrook_meadow_silverleaf"),
            0.0,
            0.0,
        );
        node.zone_id = "eastbrook".into();
        let mut entities = vec![player, node];
        let mut events = Vec::new();

        assert!(handle_interact(
            &mut entities,
            1,
            &InteractAction::TrainProfession {
                id: "herbalism".into(),
            },
            &mut events,
        ));
        assert!(handle_interact(
            &mut entities,
            1,
            &InteractAction::Gather { node_id: 2 },
            &mut events,
        ));
        assert_eq!(count_item(&entities[0].inventory, "silverleaf"), 1);
    }
}
