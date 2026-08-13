//! Talent spend / respec.

use std::collections::HashMap;

use crate::ecs::components::{ClassKit, Progress};
use crate::ecs::World;
use crate::stats::recalc_player_stats;
use woc_content::{talent, talent_tier_unlocked};
use woc_protocol::{EntityId, SimEvent};

/// Damage multiplier from learned talents (1.0 = none).
pub fn damage_multiplier_from_ranks(talents: &HashMap<String, u32>) -> f32 {
    let mut mult = 1.0;
    for (id, rank) in talents {
        if let Some(def) = talent(id) {
            if def.effect == "damage_pct" {
                mult += def.effect_value * (*rank as f32);
            }
        }
    }
    mult
}

/// Damage multiplier from learned talents (1.0 = none).
pub fn damage_multiplier(world: &World, player_id: EntityId) -> f32 {
    world
        .get::<Progress>(player_id)
        .map(|p| damage_multiplier_from_ranks(&p.talents))
        .unwrap_or(1.0)
}

fn player_rank_pairs(world: &World, player_id: EntityId) -> Vec<(String, u32)> {
    world
        .get::<Progress>(player_id)
        .map(|p| {
            p.talents
                .iter()
                .map(|(id, rank)| (id.clone(), *rank))
                .collect()
        })
        .unwrap_or_default()
}

pub fn learn(
    world: &mut World,
    player_id: EntityId,
    talent_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = talent(talent_id) else {
        events.push(SimEvent::Toast {
            message: "Unknown talent.".into(),
        });
        return false;
    };
    if world.get::<ClassKit>(player_id).is_none() {
        return false;
    };
    let class = world
        .get::<ClassKit>(player_id)
        .and_then(|k| k.class_id)
        .map(|c| c.as_str())
        .unwrap_or("");
    if def.class_id != class && def.class_id != "*" {
        events.push(SimEvent::Toast {
            message: "Wrong class for that talent.".into(),
        });
        return false;
    }
    let Some(progress) = world.get::<Progress>(player_id) else {
        return false;
    };
    if progress.talent_points == 0 {
        events.push(SimEvent::Toast {
            message: "No talent points.".into(),
        });
        return false;
    }
    let ranks = player_rank_pairs(world, player_id);
    if !talent_tier_unlocked(class, &ranks, def) {
        events.push(SimEvent::Toast {
            message: format!(
                "Tier {} locked — spend more points in lower tiers first.",
                def.tier
            ),
        });
        return false;
    }
    let rank = world
        .get::<Progress>(player_id)
        .and_then(|p| p.talents.get(talent_id).copied())
        .unwrap_or(0);
    if rank >= def.max_rank {
        events.push(SimEvent::Toast {
            message: "Talent maxed.".into(),
        });
        return false;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.talent_points -= 1;
        p.talents.insert(talent_id.to_string(), rank + 1);
    }
    recalc_player_stats(world, player_id);
    events.push(SimEvent::TalentLearned {
        player: player_id,
        talent_id: talent_id.to_string(),
        rank: rank + 1,
    });
    true
}

pub fn respec(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    if world.get::<ClassKit>(player_id).is_none() {
        return false;
    };
    let Some(progress) = world.get::<Progress>(player_id) else {
        return false;
    };
    let spent: u32 = progress.talents.values().sum();
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.talents = HashMap::new();
        p.talent_points += spent;
    }
    recalc_player_stats(world, player_id);
    events.push(SimEvent::TalentRespec { player: player_id });
    true
}

/// Grant a talent point on level-up (call from combat XP path).
pub fn on_level_up(world: &mut World, player_id: EntityId) {
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.talent_points = p.talent_points.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Progress;
    use woc_content::PlayerClass;

    #[test]
    fn learn_requires_points() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.talent_points = 1;
        }
        let mut events = Vec::new();
        assert!(learn(&mut world, 1, "warrior_cruelty", &mut events));
        assert_eq!(world.get::<Progress>(1).unwrap().talent_points, 0);
        assert_eq!(
            world
                .get::<Progress>(1)
                .unwrap()
                .talents
                .get("warrior_cruelty"),
            Some(&1)
        );
    }

    #[test]
    fn respec_refunds_points() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.talent_points = 6;
        }
        let mut events = Vec::new();
        assert!(learn(&mut world, 1, "warrior_cruelty", &mut events));
        assert!(learn(&mut world, 1, "warrior_cruelty", &mut events));
        assert!(respec(&mut world, 1, &mut events));
        assert!(world.get::<Progress>(1).unwrap().talents.is_empty());
        assert_eq!(world.get::<Progress>(1).unwrap().talent_points, 6);
    }

    #[test]
    fn tier_two_blocked_until_five_tier_one_points() {
        let mut world = World::new();
        crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
        if let Some(p) = world.get_mut::<Progress>(1) {
            p.talent_points = 6;
        }
        let mut events = Vec::new();

        assert!(
            !learn(&mut world, 1, "warrior_vitality", &mut events),
            "tier 2 should be locked with no tier-1 spend"
        );
        assert!(events.iter().any(|e| matches!(e, SimEvent::Toast { .. })));

        for _ in 0..5 {
            assert!(learn(&mut world, 1, "warrior_cruelty", &mut events));
        }
        assert!(learn(&mut world, 1, "warrior_vitality", &mut events));
        assert_eq!(
            world
                .get::<Progress>(1)
                .unwrap()
                .talents
                .get("warrior_vitality"),
            Some(&1)
        );
    }
}
