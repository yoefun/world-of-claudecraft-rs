//! `WorldHost` implementation for `Sim` (multi-player aware).

use crate::interaction::handle_interact;
use crate::sim::Sim;
use woc_protocol::{EntityId, InteractAction, PlayerIntent, SimEvent, TickSnapshot, WorldHost};

impl WorldHost for Sim {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent) {
        if self.entities.iter().any(|e| e.id == player_id) {
            self.intents.insert(player_id, intent);
        }
    }

    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction) {
        handle_interact(
            &mut self.entities,
            player_id,
            target_id,
            action,
            &mut self.events,
        );
    }

    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>) {
        self.tick_all()
    }

    fn snapshot_for(&self, player_id: EntityId) -> TickSnapshot {
        self.snapshot_for_player(player_id)
    }
}
