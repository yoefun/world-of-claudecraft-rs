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
        if self.world.contains(player_id) {
            self.intents.insert(player_id, intent);
        }
    }

    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction) {
        match action {
            InteractAction::SummonPet => {
                let _ = summon_pet(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::DismissPet => {
                let _ = dismiss_pet(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::UseHearthstone => {
                let _ = crate::zones::use_hearthstone(
                    &mut self.world,
                    player_id,
                    self.tick,
                    &mut self.events,
                );
            }
            InteractAction::AbandonQuest { quest_id } => {
                let _ = crate::quests::abandon_quest(
                    &mut self.world,
                    player_id,
                    &quest_id,
                    &mut self.events,
                );
            }
            InteractAction::ShareQuest { quest_id } => {
                let _ = crate::quests::share_quest(
                    &mut self.world,
                    &self.parties,
                    player_id,
                    &quest_id,
                    &mut self.events,
                );
            }
            InteractAction::LearnTalent { talent_id } => {
                let _ = talents::learn(&mut self.world, player_id, &talent_id, &mut self.events);
            }
            InteractAction::RespecTalents => {
                let _ = talents::respec(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::BankDeposit { bag_slot, count } => {
                let _ = bank::deposit(
                    &mut self.world,
                    player_id,
                    bag_slot,
                    count,
                    &mut self.events,
                );
            }
            InteractAction::BankWithdraw { bank_slot, count } => {
                let _ = bank::withdraw(
                    &mut self.world,
                    player_id,
                    bank_slot,
                    count,
                    &mut self.events,
                );
            }
            InteractAction::BankDepositCopper { amount } => {
                let _ = bank::deposit_copper(&mut self.world, player_id, amount, &mut self.events);
            }
            InteractAction::BankWithdrawCopper { amount } => {
                let _ = bank::withdraw_copper(&mut self.world, player_id, amount, &mut self.events);
            }
            InteractAction::MailSend {
                to_name,
                copper,
                bag_slot,
                count,
            } => {
                let _ = self.mail.send(
                    &mut self.world,
                    player_id,
                    &to_name,
                    copper,
                    bag_slot,
                    count,
                    self.tick,
                    &self.directory,
                    &mut self.events,
                );
            }
            InteractAction::MailCollect { mail_id } => {
                let _ = self
                    .mail
                    .collect(&mut self.world, player_id, mail_id, &mut self.events);
            }
            InteractAction::MailReturn { mail_id } => {
                let _ = self
                    .mail
                    .return_mail(&mut self.world, player_id, mail_id, &mut self.events);
            }
            InteractAction::MarketList {
                bag_slot,
                count,
                price,
            } => {
                let _ = self.market.list_item(
                    &mut self.world,
                    player_id,
                    bag_slot,
                    count,
                    price,
                    self.tick,
                    &mut self.events,
                );
            }
            InteractAction::MarketBuy { listing_id } => {
                let _ = self.market.buy(
                    &mut self.world,
                    &mut self.mail,
                    player_id,
                    listing_id,
                    &mut self.events,
                );
            }
            InteractAction::MarketCancel { listing_id } => {
                let _ = self.market.cancel(
                    &mut self.world,
                    &mut self.mail,
                    player_id,
                    listing_id,
                    &mut self.events,
                );
            }
            InteractAction::DuelChallenge => {
                let _ = challenge_duel(&mut self.pvp, &self.world, player_id, target_id);
            }
            InteractAction::DuelAccept => {
                let _ =
                    accept_pending_duel(&mut self.pvp, &self.world, player_id, &mut self.events);
            }
            InteractAction::TogglePvp => {
                let _ = toggle_pvp(&mut self.world, player_id);
            }
            InteractAction::EnterPortal { zone_id } => {
                let _ = enter_portal(&mut self.world, player_id, &zone_id, &mut self.events);
            }
            InteractAction::EnterDungeon { dungeon_id } => {
                let _ = enter_dungeon(
                    &mut self.world,
                    &self.parties,
                    player_id,
                    &dungeon_id,
                    &mut self.events,
                );
            }
            InteractAction::EnterDelve { delve_id } => {
                let _ = enter_delve(&mut self.world, player_id, &delve_id, &mut self.events);
            }
            InteractAction::AdvanceDelve => {
                let _ = try_advance_delve(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::LeaveInstance => {
                let _ = leave_instance(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::ToggleStealth => {
                crate::combat::toggle_stealth(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::CycleStance => {
                crate::combat::cycle_stance(&mut self.world, player_id, &mut self.events);
            }
            InteractAction::ToggleForm => {
                crate::combat::toggle_form(&mut self.world, player_id, &mut self.events);
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
                    if self.parties.set_loot_mode(player_id, m) {
                        if let Some(pid) = self.parties.party_id(player_id) {
                            self.loot_rules.set_mode(pid, m);
                        }
                        self.events.push(SimEvent::Toast {
                            message: format!("Loot mode: {}.", m.as_str()),
                        });
                    }
                }
            }
            InteractAction::LootCorpse { target_id } => {
                let _ = crate::combat::claim_loot_target(
                    player_id,
                    target_id,
                    &mut self.world,
                    &mut self.events,
                    &self.loot_rules,
                );
            }
            ref other
                if matches!(
                    other,
                    InteractAction::TrainProfession { .. }
                        | InteractAction::Gather { .. }
                        | InteractAction::Craft { .. }
                        | InteractAction::Skin { .. }
                        | InteractAction::Disenchant { .. }
                        | InteractAction::ApplyEnchant { .. }
                ) =>
            {
                let _ = professions::handle_interact(
                    &mut self.world,
                    player_id,
                    target_id,
                    other,
                    self.tick,
                    &mut self.rng,
                    &mut self.events,
                );
            }
            other => {
                handle_interact(
                    &mut self.world,
                    player_id,
                    target_id,
                    other,
                    self.tick,
                    &mut self.events,
                );
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
        let _ = self.loot_rules.roll(
            loot_id,
            player_id,
            choice,
            &mut self.rng,
            &mut self.world,
            &mut self.events,
        );
    }
}
