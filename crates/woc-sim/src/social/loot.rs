//! Party loot modes: FFA and Need/Greed.

use std::collections::HashMap;

use crate::entity::{grant_into, Entity};
use crate::rng::Rng;
use crate::social::party::{PartyRoster, PARTY_CREDIT_RANGE};
use woc_protocol::{EntityId, EntityKind, PendingLootSnapshot, SimEvent};

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
    /// party_id → mode (mirrored from party leader SetLootMode for convenience)
    pub modes: HashMap<u32, LootMode>,
    pub pending: Vec<PendingLoot>,
}

impl LootRules {
    pub fn mode_for_party(&self, party_id: u32) -> LootMode {
        self.modes.get(&party_id).copied().unwrap_or(LootMode::Ffa)
    }

    pub fn set_mode(&mut self, party_id: u32, mode: LootMode) {
        self.modes.insert(party_id, mode);
    }

    pub fn is_pending(&self, loot_id: EntityId) -> bool {
        self.pending.iter().any(|p| p.loot_id == loot_id)
    }

    /// Snapshot pending rolls relevant to `player` (eligible only).
    pub fn snapshot_for(&self, player: EntityId) -> Vec<PendingLootSnapshot> {
        self.pending
            .iter()
            .filter(|p| p.eligible.contains(&player))
            .map(|p| PendingLootSnapshot {
                loot_id: p.loot_id,
                item_id: p.item_id.clone(),
                copper: p.copper,
                rolled: p.rolls.contains_key(&player),
            })
            .collect()
    }

    /// After a loot pile spawns from a mob kill, start Need/Greed when the
    /// killer's party is in that mode and at least two eligible members are near.
    pub fn maybe_start_party_roll(
        &mut self,
        parties: &PartyRoster,
        entities: &[Entity],
        killer: EntityId,
        loot_id: EntityId,
        events: &mut Vec<SimEvent>,
    ) {
        let Some(party_id) = parties.party_id(killer) else {
            return;
        };
        let mode = parties
            .loot_mode(killer)
            .and_then(|s| LootMode::parse(&s))
            .or_else(|| self.modes.get(&party_id).copied())
            .unwrap_or(LootMode::Ffa);
        self.modes.insert(party_id, mode);
        if mode != LootMode::NeedGreed {
            return;
        }
        let Some(loot) = entities
            .iter()
            .find(|e| e.id == loot_id && e.kind == EntityKind::Loot && e.alive)
        else {
            return;
        };
        let item_id = loot.loot_item.clone().unwrap_or_default();
        let copper = loot.loot_copper;
        if item_id.is_empty() && copper == 0 {
            return;
        }
        let eligible = eligible_near_loot(parties, entities, killer, loot);
        if eligible.len() < 2 {
            return;
        }
        events.push(SimEvent::Toast {
            message: format!(
                "Need/Greed: {} ({}c) — press 1 Need / 2 Greed / 3 Pass",
                if item_id.is_empty() {
                    "copper".into()
                } else {
                    item_id.clone()
                },
                copper
            ),
        });
        self.start_roll(loot_id, item_id, copper, eligible);
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
            events.push(SimEvent::Toast {
                message: "Everyone passed — loot discarded.".into(),
            });
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

fn eligible_near_loot(
    parties: &PartyRoster,
    entities: &[Entity],
    killer: EntityId,
    loot: &Entity,
) -> Vec<EntityId> {
    let Some(mut members) = parties.members_of(killer) else {
        return vec![killer];
    };
    members.retain(|id| {
        entities
            .iter()
            .find(|e| e.id == *id && e.kind == EntityKind::Player && e.alive)
            .map(|mate| {
                let dx = mate.x - loot.x;
                let dz = mate.z - loot.z;
                (dx * dx + dz * dz).sqrt() <= PARTY_CREDIT_RANGE
                    && mate.instance_id == loot.instance_id
            })
            .unwrap_or(false)
    });
    if !members.contains(&killer) {
        members.push(killer);
    }
    members
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
    use crate::social::party::PartyRoster;
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
        assert!(events
            .iter()
            .any(|e| matches!(e, SimEvent::LootAwarded { winner: 2, .. })));
        assert!(!entities[2].alive);
    }

    #[test]
    fn need_greed_party_starts_roll_for_two() {
        let mut entities = vec![
            create_player(1, "A", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "B", PlayerClass::Mage, 1.0, 0.0),
            Entity::blank(50, EntityKind::Loot, "loot", None, 0.5, 0.0),
        ];
        entities[2].alive = true;
        entities[2].loot_item = Some("wolf_fang".into());
        entities[2].loot_copper = 4;
        let mut roster = PartyRoster::new();
        roster.invite(1, "B", &entities);
        roster.accept(2, &entities);
        assert!(roster.set_loot_mode(1, LootMode::NeedGreed));
        let mut rules = LootRules::default();
        let mut events = Vec::new();
        rules.maybe_start_party_roll(&roster, &entities, 1, 50, &mut events);
        assert_eq!(rules.pending.len(), 1);
        assert_eq!(rules.pending[0].eligible.len(), 2);
        assert!(rules.is_pending(50));
        let snap = rules.snapshot_for(1);
        assert_eq!(snap.len(), 1);
        assert!(!snap[0].rolled);
    }

    #[test]
    fn ffa_does_not_start_roll() {
        let mut entities = vec![
            create_player(1, "A", PlayerClass::Warrior, 0.0, 0.0),
            create_player(2, "B", PlayerClass::Mage, 1.0, 0.0),
            Entity::blank(50, EntityKind::Loot, "loot", None, 0.5, 0.0),
        ];
        entities[2].alive = true;
        entities[2].loot_item = Some("wolf_fang".into());
        let mut roster = PartyRoster::new();
        roster.invite(1, "B", &entities);
        roster.accept(2, &entities);
        let mut rules = LootRules::default();
        let mut events = Vec::new();
        rules.maybe_start_party_roll(&roster, &entities, 1, 50, &mut events);
        assert!(rules.pending.is_empty());
    }
}
