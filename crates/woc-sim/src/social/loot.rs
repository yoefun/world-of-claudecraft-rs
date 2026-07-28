//! Party loot modes: FFA and Need/Greed.

use std::collections::HashMap;

use crate::entity::{grant_into, Entity};
use crate::rng::Rng;
use woc_protocol::{EntityId, EntityKind, SimEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootMode {
    Ffa,
    NeedGreed,
}

impl LootMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ffa" => Some(Self::Ffa),
            "need_greed" | "need-greed" => Some(Self::NeedGreed),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ffa => "ffa",
            Self::NeedGreed => "need_greed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollChoice {
    Need,
    Greed,
    Pass,
}

#[derive(Debug, Clone)]
pub struct PendingLoot {
    pub loot_id: EntityId,
    pub item_id: String,
    pub copper: u32,
    pub eligible: Vec<EntityId>,
    pub rolls: HashMap<EntityId, (RollChoice, u32)>,
}

#[derive(Debug, Default)]
pub struct LootRules {
    /// party_id → mode
    pub modes: HashMap<u32, LootMode>,
    pub pending: Vec<PendingLoot>,
}

impl LootRules {
    pub fn mode_for_party(&self, party_id: u32) -> LootMode {
        self.modes
            .get(&party_id)
            .copied()
            .unwrap_or(LootMode::Ffa)
    }

    pub fn set_mode(&mut self, party_id: u32, mode: LootMode) {
        self.modes.insert(party_id, mode);
    }

    /// Start a Need/Greed roll for a loot entity among eligible party members.
    pub fn start_roll(
        &mut self,
        loot_id: EntityId,
        item_id: String,
        copper: u32,
        eligible: Vec<EntityId>,
    ) {
        if eligible.is_empty() {
            return;
        }
        self.pending.push(PendingLoot {
            loot_id,
            item_id,
            copper,
            eligible,
            rolls: HashMap::new(),
        });
    }

    pub fn roll(
        &mut self,
        loot_id: EntityId,
        player: EntityId,
        choice: RollChoice,
        rng: &mut Rng,
        entities: &mut [Entity],
        events: &mut Vec<SimEvent>,
    ) -> bool {
        let Some(idx) = self.pending.iter().position(|p| p.loot_id == loot_id) else {
            return false;
        };
        if !self.pending[idx].eligible.contains(&player) {
            return false;
        }
        if self.pending[idx].rolls.contains_key(&player) {
            return false;
        }
        let roll = if matches!(choice, RollChoice::Pass) {
            0
        } else {
            (rng.next_u32() % 100) + 1
        };
        let choice_str = match choice {
            RollChoice::Need => "need",
            RollChoice::Greed => "greed",
            RollChoice::Pass => "pass",
        };
        events.push(SimEvent::LootRoll {
            loot_id,
            player,
            choice: choice_str.into(),
            roll,
        });
        self.pending[idx].rolls.insert(player, (choice, roll));

        if self.pending[idx].rolls.len() >= self.pending[idx].eligible.len() {
            self.resolve(idx, entities, events);
        }
        true
    }

    fn resolve(&mut self, idx: usize, entities: &mut [Entity], events: &mut Vec<SimEvent>) {
        let pending = self.pending.remove(idx);
        let winner = pick_winner(&pending);
        let Some(winner) = winner else {
            // All passed — despawn loot.
            if let Some(loot) = entities.iter_mut().find(|e| e.id == pending.loot_id) {
                loot.alive = false;
            }
            return;
        };
        if let Some(player) = entities.iter_mut().find(|e| e.id == winner) {
            if !pending.item_id.is_empty() {
                let _ = grant_into(&mut player.inventory, &pending.item_id, 1);
            }
            player.copper = player.copper.saturating_add(pending.copper);
        }
        if let Some(loot) = entities.iter_mut().find(|e| e.id == pending.loot_id) {
            loot.alive = false;
            loot.loot_item = None;
            loot.loot_copper = 0;
        }
        events.push(SimEvent::LootAwarded {
            loot_id: pending.loot_id,
            winner,
            item_id: pending.item_id,
        });
    }
}

fn pick_winner(pending: &PendingLoot) -> Option<EntityId> {
    let needs: Vec<_> = pending
        .rolls
        .iter()
        .filter(|(_, (c, _))| *c == RollChoice::Need)
        .collect();
    let pool = if !needs.is_empty() {
        needs
    } else {
        pending
            .rolls
            .iter()
            .filter(|(_, (c, _))| *c == RollChoice::Greed)
            .collect()
    };
    pool.into_iter()
        .max_by_key(|(_, (_, r))| *r)
        .map(|(id, _)| *id)
}

/// FFA: first eligible party member in range may pick up (handled by caller).
pub fn is_loot_entity(e: &Entity) -> bool {
    e.kind == EntityKind::Loot && e.alive
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::create_player;
    use crate::rng::Rng;
    use woc_content::PlayerClass;
    use woc_protocol::EntityKind;

    #[test]
    fn need_beats_greed() {
        let mut rules = LootRules::default();
        rules.start_roll(99, "wolf_fang".into(), 5, vec![1, 2]);
        let mut entities = vec![
            create_player(1, "A", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "B", PlayerClass::Mage, 1.0, 0.0),
            Entity::blank(99, EntityKind::Loot, "loot", None, 0.0, 0.0),
        ];
        entities[2].alive = true;
        entities[2].loot_item = Some("wolf_fang".into());
        let mut rng = Rng::new(1);
        let mut events = Vec::new();
        assert!(rules.roll(
            99,
            1,
            RollChoice::Greed,
            &mut rng,
            &mut entities,
            &mut events
        ));
        assert!(rules.roll(
            99,
            2,
            RollChoice::Need,
            &mut rng,
            &mut entities,
            &mut events
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::LootAwarded {
                winner: 2,
                ..
            }
        )));
        assert!(!entities[2].alive);
    }
}
