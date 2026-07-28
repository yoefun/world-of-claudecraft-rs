//! `WorldHost` implementation for `Sim` (single-player / primary player view).

use crate::interaction::handle_interact;
use crate::sim::Sim;
use woc_protocol::{EntityId, InteractAction, PlayerIntent, SimEvent, TickSnapshot, WorldHost};

impl WorldHost for Sim {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent) {
        if player_id == self.player_id {
            self.pending_intent = intent;
        }
    }

    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction) {
        let mut xp = self.player_xp;
        let mut copper = self.copper;
        handle_interact(
            &mut self.entities,
            player_id,
            target_id,
            action,
            &mut xp,
            &mut copper,
            &mut self.events,
        );
        self.player_xp = xp;
        self.copper = copper;
    }

    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>) {
        let intent = self.pending_intent;
        self.tick(intent)
    }

    fn snapshot_for(&self, _player_id: EntityId) -> TickSnapshot {
        self.snapshot()
    }
}
