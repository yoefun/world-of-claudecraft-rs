//! Faction standing: award, snapshot, vendor gates.

use std::collections::HashMap;

use crate::ecs::components::Reputation;
use crate::ecs::World;
use woc_content::{
    clamp_reputation, discounted_price, faction, standing_from_value, vendor_discount_pct, NpcDef,
    Standing, FACTIONS,
};
use woc_protocol::{EntityId, ReputationSnapshot, SimEvent};

pub fn value_of(world: &World, player_id: EntityId, faction_id: &str) -> i32 {
    world
        .get::<Reputation>(player_id)
        .and_then(|r| r.values.get(faction_id).copied())
        .unwrap_or(0)
}

pub fn standing_of(world: &World, player_id: EntityId, faction_id: &str) -> Standing {
    standing_from_value(value_of(world, player_id, faction_id))
}

pub fn npc_standing(world: &World, player_id: EntityId, def: &NpcDef) -> Standing {
    match def.faction {
        Some(id) => standing_of(world, player_id, id),
        None => Standing::Neutral,
    }
}

pub fn award(
    world: &mut World,
    player_id: EntityId,
    faction_id: &str,
    amount: i32,
    events: &mut Vec<SimEvent>,
) -> Option<i32> {
    if amount == 0 || faction(faction_id).is_none() {
        return None;
    }
    if world.get::<Reputation>(player_id).is_none() {
        world.insert(player_id, Reputation::default());
    }
    let rep = world.get_mut::<Reputation>(player_id)?;
    let before = rep.values.get(faction_id).copied().unwrap_or(0);
    let total = clamp_reputation(before.saturating_add(amount));
    if total == before {
        return Some(total);
    }
    let before_rank = standing_from_value(before);
    let after_rank = standing_from_value(total);
    rep.values.insert(faction_id.to_string(), total);
    let name = faction(faction_id).map(|f| f.name).unwrap_or(faction_id);
    events.push(SimEvent::ReputationChanged {
        player: player_id,
        faction_id: faction_id.to_string(),
        delta: amount,
        total,
        standing: after_rank.as_str().to_string(),
    });
    let signed = if amount > 0 {
        format!("+{amount}")
    } else {
        amount.to_string()
    };
    events.push(SimEvent::Toast {
        message: format!("{name} {signed} ({})", after_rank.display_name()),
    });
    if after_rank != before_rank {
        events.push(SimEvent::Toast {
            message: format!("{name}: {}", after_rank.display_name()),
        });
    }
    Some(total)
}

pub fn on_mob_killed(
    world: &mut World,
    player_id: EntityId,
    mob_template_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(award_def) = woc_content::mob(mob_template_id).and_then(|m| m.kill_reputation) else {
        return;
    };
    award(
        world,
        player_id,
        award_def.faction_id,
        award_def.amount,
        events,
    );
}

pub fn snapshot(world: &World, player_id: EntityId) -> Vec<ReputationSnapshot> {
    let values: HashMap<String, i32> = world
        .get::<Reputation>(player_id)
        .map(|r| r.values.clone())
        .unwrap_or_default();
    FACTIONS
        .iter()
        .map(|f| {
            let value = values.get(f.id).copied().unwrap_or(0);
            ReputationSnapshot {
                faction_id: f.id.to_string(),
                name: f.name.to_string(),
                value,
                standing: standing_from_value(value).as_str().to_string(),
            }
        })
        .collect()
}

pub fn from_saved(values: HashMap<String, i32>) -> Reputation {
    let mut cleaned = HashMap::new();
    for (id, value) in values {
        if faction(&id).is_some() {
            cleaned.insert(id, clamp_reputation(value));
        }
    }
    Reputation { values: cleaned }
}

pub fn vendor_price(base: u32, standing: Standing) -> u32 {
    discounted_price(base, standing)
}

pub fn vendor_discount(standing: Standing) -> u32 {
    vendor_discount_pct(standing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::spawn::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn award_clamps_and_ranks_up() {
        let mut world = World::new();
        create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        assert_eq!(
            award(&mut world, 1, "eastbrook_watch", 500, &mut events),
            Some(500)
        );
        assert_eq!(
            standing_of(&world, 1, "eastbrook_watch"),
            Standing::Friendly
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::ReputationChanged { total: 500, .. })));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message.contains("Friendly")
        )));
        award(&mut world, 1, "eastbrook_watch", 20_000, &mut events);
        assert_eq!(
            value_of(&world, 1, "eastbrook_watch"),
            woc_content::STANDING_CAP
        );
    }

    #[test]
    fn unknown_faction_is_ignored() {
        let mut world = World::new();
        create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        let mut events = Vec::new();
        assert!(award(&mut world, 1, "nope", 10, &mut events).is_none());
        assert!(events.is_empty());
    }
}
