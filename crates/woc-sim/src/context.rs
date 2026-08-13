//! SimContext seam: emit + entity lookup/mutate without the full `Sim` facade.

use std::collections::HashMap;

use crate::entity::Entity;
use crate::rng::Rng;
use woc_protocol::{EntityId, SimEvent};

/// Callback bag held during a tick / interaction.
///
/// Leaf modules should prefer `&mut SimContext` over reaching into `Sim`.
pub struct SimContext<'a> {
    pub events: &'a mut Vec<SimEvent>,
    pub entities: &'a mut [Entity],
    pub by_id: &'a HashMap<EntityId, usize>,
    pub rng: &'a mut Rng,
    pub next_id: &'a mut EntityId,
}

impl<'a> SimContext<'a> {
    pub fn emit(&mut self, event: SimEvent) {
        self.events.push(event);
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        let i = *self.by_id.get(&id)?;
        self.entities.get(i).filter(|e| e.id == id)
    }

    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        let i = *self.by_id.get(&id)?;
        self.entities.get_mut(i).filter(|e| e.id == id)
    }

    pub fn player_ids(&self) -> Vec<EntityId> {
        self.entities
            .iter()
            .filter(|e| e.kind == woc_protocol::EntityKind::Player && e.alive)
            .map(|e| e.id)
            .collect()
    }
}

/// Locked tick phase names (order is part of the determinism contract).
/// Do not reorder without updating this list and the hash test in `sim.rs`.
pub const TICK_PHASES: &[&str] = &[
    "apply_intents_motion",
    "player_combat",
    "mob_ai_combat",
    "kill_rewards",
    "loot_pickup",
    "build_snapshot",
];

pub fn tick_phase_fingerprint() -> u64 {
    // Simple FNV-1a over phase name bytes — stable across platforms.
    let mut hash: u64 = 0xcbf29ce484222325;
    for phase in TICK_PHASES {
        for b in phase.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
