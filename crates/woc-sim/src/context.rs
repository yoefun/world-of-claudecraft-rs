//! SimContext seam: emit + World lookup without the full `Sim` facade.

use crate::ecs::components::{Health, Identity};
use crate::ecs::World;
use crate::rng::Rng;
use woc_protocol::{EntityId, EntityKind, SimEvent};

/// Callback bag held during a tick / interaction.
///
/// Leaf modules should prefer `&mut SimContext` over reaching into `Sim`.
pub struct SimContext<'a> {
    pub events: &'a mut Vec<SimEvent>,
    pub world: &'a mut World,
    pub rng: &'a mut Rng,
}

impl<'a> SimContext<'a> {
    pub fn emit(&mut self, event: SimEvent) {
        self.events.push(event);
    }

    pub fn player_ids(&self) -> Vec<EntityId> {
        self.world
            .live_ids()
            .filter(|&id| {
                self.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Player)
                    && self
                        .world
                        .get::<Health>(id)
                        .map(|h| h.alive)
                        .unwrap_or(false)
            })
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
