//! `WorldHost` implementation for `Sim` (multi-player aware).

use crate::bank;
use crate::delves::{enter_delve, try_advance_delve};
use crate::instances::{enter_dungeon, leave_instance};
use crate::interaction::handle_interact;
use crate::pet::{dismiss_pet, summon_pet};
use crate::professions;
use crate::pvp::{accept_pending_duel, challenge_duel, toggle_pvp};
use crate::sim::Sim;
use crate::social::{LootMode, RollChoice};
use crate::talents;
use crate::zones::enter_portal;
use woc_protocol::{EntityId, InteractAction, PlayerIntent, SimEvent, TickSnapshot, WorldHost};

impl WorldHost for Sim {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent) {
        if self.entities.iter().any(|e| e.id == player_id) {
            self.intents.insert(player_id, intent);
        }
    }

    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction) {
        match action {
            InteractAction::SummonPet => {
                self.rebuild_world();
                let _ = summon_pet(
                    &mut self.world,
                    &mut self.entities,
                    &mut self.next_id,
                    player_id,
                    &mut self.events,
                );
                self.reindex();
            }
            InteractAction::DismissPet => {
                self.rebuild_world();
                let _ = dismiss_pet(
                    &mut self.world,
                    &mut self.entities,
                    player_id,
                    &mut self.events,
                );
                self.reindex();
            }
            InteractAction::LearnTalent { talent_id } => {
                self.rebuild_world();
                let _ = talents::learn(&mut self.world, player_id, &talent_id, &mut self.events);
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::RespecTalents => {
                self.rebuild_world();
                let _ = talents::respec(&mut self.world, player_id, &mut self.events);
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::BankDeposit { bag_slot, count } => {
                self.rebuild_world();
                let _ = bank::deposit(
                    &mut self.world,
                    player_id,
                    bag_slot,
                    count,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::BankWithdraw { bank_slot, count } => {
                self.rebuild_world();
                let _ = bank::withdraw(
                    &mut self.world,
                    player_id,
                    bank_slot,
                    count,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::MailSend {
                to_name,
                copper,
                bag_slot,
                count,
            } => {
                self.rebuild_world();
                let _ = self.mail.send(
                    &mut self.world,
                    player_id,
                    &to_name,
                    copper,
                    bag_slot,
                    count,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::MailCollect { mail_id } => {
                self.rebuild_world();
                let _ = self
                    .mail
                    .collect(&mut self.world, player_id, mail_id, &mut self.events);
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::MarketList {
                bag_slot,
                count,
                price,
            } => {
                self.rebuild_world();
                let _ = self.market.list_item(
                    &mut self.world,
                    player_id,
                    bag_slot,
                    count,
                    price,
                    self.tick,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::MarketBuy { listing_id } => {
                self.rebuild_world();
                let _ = self.market.buy(
                    &mut self.world,
                    &mut self.mail,
                    player_id,
                    listing_id,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::MarketCancel { listing_id } => {
                self.rebuild_world();
                let _ = self.market.cancel(
                    &mut self.world,
                    &mut self.mail,
                    player_id,
                    listing_id,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::DuelChallenge => {
                self.rebuild_world();
                let _ = challenge_duel(&mut self.pvp, &self.world, player_id, target_id);
            }
            InteractAction::DuelAccept => {
                self.rebuild_world();
                let _ = accept_pending_duel(
                    &mut self.pvp,
                    &self.world,
                    player_id,
                    &mut self.events,
                );
            }
            InteractAction::TogglePvp => {
                self.rebuild_world();
                let _ = toggle_pvp(&mut self.world, player_id);
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            InteractAction::EnterPortal { zone_id } => {
                let _ = enter_portal(&mut self.entities, player_id, &zone_id, &mut self.events);
            }
            InteractAction::EnterDungeon { dungeon_id } => {
                let _ = enter_dungeon(
                    &mut self.entities,
                    &mut self.next_id,
                    &self.parties,
                    player_id,
                    &dungeon_id,
                    &mut self.events,
                );
            }
            InteractAction::EnterDelve { delve_id } => {
                if enter_delve(&mut self.entities, player_id, &delve_id, &mut self.events) {
                    self.next_id = self
                        .entities
                        .iter()
                        .map(|entity| entity.id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .max(self.next_id);
                }
            }
            InteractAction::AdvanceDelve => {
                if try_advance_delve(&mut self.entities, player_id, &mut self.events) {
                    self.next_id = self
                        .entities
                        .iter()
                        .map(|entity| entity.id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .max(self.next_id);
                }
            }
            InteractAction::LeaveInstance => {
                let _ = leave_instance(&mut self.entities, player_id, &mut self.events);
            }
            InteractAction::LootNeed { loot_id } => {
                self.roll_loot(loot_id, player_id, RollChoice::Need);
            }
            InteractAction::LootGreed { loot_id } => {
                self.roll_loot(loot_id, player_id, RollChoice::Greed);
            }
            InteractAction::LootPass { loot_id } => {
                self.roll_loot(loot_id, player_id, RollChoice::Pass);
            }
            InteractAction::SetLootMode { mode } => {
                if let Some(m) = LootMode::parse(&mode) {
                    let _ = self.parties.set_loot_mode(player_id, m);
                }
            }
            ref other
                if matches!(
                    other,
                    InteractAction::TrainProfession { .. }
                        | InteractAction::Gather { .. }
                        | InteractAction::Craft { .. }
                ) =>
            {
                self.rebuild_world();
                let _ = professions::handle_interact(
                    &mut self.world,
                    player_id,
                    other,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
            other => {
                self.rebuild_world();
                handle_interact(
                    &mut self.world,
                    player_id,
                    target_id,
                    other,
                    &mut self.events,
                );
                crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
            }
        }
    }

    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>) {
        self.tick_all()
    }

    fn snapshot_for(&self, player_id: EntityId) -> TickSnapshot {
        self.snapshot_for_player(player_id)
    }
}

impl Sim {
    fn roll_loot(&mut self, loot_id: EntityId, player_id: EntityId, choice: RollChoice) {
        self.rebuild_world();
        let _ = self.loot_rules.roll(
            loot_id,
            player_id,
            choice,
            &mut self.rng,
            &mut self.world,
            &mut self.events,
        );
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        if !self.world.contains(loot_id) {
            if let Some(loot) = self.entities.iter_mut().find(|e| e.id == loot_id) {
                loot.alive = false;
                loot.loot_item = None;
                loot.loot_copper = 0;
            }
        }
    }
}
