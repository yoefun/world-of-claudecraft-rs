//! Talent spend / respec.

use std::collections::HashMap;

use crate::entity::Entity;
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
pub fn damage_multiplier(player: &Entity) -> f32 {
    damage_multiplier_from_ranks(&player.talents)
}

fn player_rank_pairs(player: &Entity) -> Vec<(String, u32)> {
    player
        .talents
        .iter()
        .map(|(id, rank)| (id.clone(), *rank))
        .collect()
}

pub fn learn(
    entities: &mut [Entity],
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
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    let class = player.class_id.map(|c| c.as_str()).unwrap_or("");
    if def.class_id != class && def.class_id != "*" {
        events.push(SimEvent::Toast {
            message: "Wrong class for that talent.".into(),
        });
        return false;
    }
    if player.talent_points == 0 {
        events.push(SimEvent::Toast {
            message: "No talent points.".into(),
        });
        return false;
    }
    let ranks = player_rank_pairs(player);
    if !talent_tier_unlocked(class, &ranks, def) {
        events.push(SimEvent::Toast {
            message: format!(
                "Tier {} locked — spend more points in lower tiers first.",
                def.tier
            ),
        });
        return false;
    }
    let rank = player.talents.get(talent_id).copied().unwrap_or(0);
    if rank >= def.max_rank {
        events.push(SimEvent::Toast {
            message: "Talent maxed.".into(),
        });
        return false;
    }
    player.talent_points -= 1;
    player.talents.insert(talent_id.to_string(), rank + 1);
    recalc_player_stats(player);
    events.push(SimEvent::TalentLearned {
        player: player_id,
        talent_id: talent_id.to_string(),
        rank: rank + 1,
    });
    true
}

pub fn respec(entities: &mut [Entity], player_id: EntityId, events: &mut Vec<SimEvent>) -> bool {
    let Some(player) = entities.iter_mut().find(|e| e.id == player_id) else {
        return false;
    };
    let spent: u32 = player.talents.values().sum();
    player.talents = HashMap::new();
    player.talent_points += spent;
    recalc_player_stats(player);
    events.push(SimEvent::TalentRespec { player: player_id });
    true
}

/// Grant a talent point on level-up (call from combat XP path).
pub fn on_level_up(player: &mut Entity) {
    player.talent_points = player.talent_points.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use woc_content::PlayerClass;

    #[test]
    fn learn_and_respec_warrior_talent() {
        let mut entities = vec![create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0)];
        entities[0].talent_points = 1;
        let mut events = Vec::new();
        assert!(learn(&mut entities, 1, "warrior_cruelty", &mut events));
        assert!((damage_multiplier(&entities[0]) - 1.05).abs() < 0.001);
        assert!(respec(&mut entities, 1, &mut events));
        assert_eq!(entities[0].talent_points, 1);
        assert!((damage_multiplier(&entities[0]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn tier_two_blocked_until_five_tier_one_points() {
        let mut entities = vec![create_player(1, "Ada", PlayerClass::Warrior, 0.0, 0.0)];
        entities[0].talent_points = 6;
        let mut events = Vec::new();

        assert!(
            !learn(&mut entities, 1, "warrior_vitality", &mut events),
            "tier 2 should be locked with no tier-1 spend"
        );
        assert!(events.iter().any(|e| matches!(e, SimEvent::Toast { .. })));

        for _ in 0..5 {
            assert!(learn(&mut entities, 1, "warrior_cruelty", &mut events));
        }
        assert!(learn(&mut entities, 1, "warrior_vitality", &mut events));
        assert_eq!(entities[0].talents.get("warrior_vitality"), Some(&1));
    }
}
