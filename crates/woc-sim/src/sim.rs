//! Sim coordinator: tick loop, intents, snapshots.
//!
//! # Tick phases (locked order — do not reorder)
//!
//! See [`crate::context::TICK_PHASES`]. Actual `tick_all` execution:
//! 1. `apply_intents_motion` — per-player intent + motion
//! 2. `player_combat` — per-player combat
//! 3. `pet_ai` — summoned pet follow / attack
//! 4. `mob_ai_combat` — mob AI + mob combat
//! 5. `aura_decay` — DoT/HoT ticks, aura expiry, mob respawn timers
//! 6. `kill_rewards` — XP/quest/deed credit, loot spawn, Need/Greed, death finalize
//! 7. `pvp_and_market` — duel resolve + auction expiry
//! 8. `loot_pickup` — proximity pickup for all players
//! 9. `profession_casts` — complete ready gather/craft/skin/enchant casts
//! 10. `build_snapshot` — snapshot for primary/`snapshot_for` viewer

use std::collections::{HashMap, HashSet};

use crate::combat::{
    collect_pending_mob_kills, grant_xp, spawn_mob_loot, tick_auras, try_pickup_loot,
    update_mob_combat, update_player_combat,
};
use crate::context::SimContext;
use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Health, Hearth, Identity, InstanceAt, LootPile, LootTable,
    Motion, Owner, Progress, Transform,
};
use crate::ecs::World;
use crate::interaction::{npc_session_snapshot, vendor_snapshot};
use crate::mob::{tick_mob_respawns, update_mob_ai};
use crate::pet::{dismiss_pet, tick_pets};
use crate::player_motion::step_player_motion;
use crate::quests::{
    credit_explore, on_mob_killed, quest_log_entries, refresh_daily_quests, tick_escorts,
};
use crate::rng::Rng;
use crate::social::chat::{handle_chat, ChatEffect};
use crate::social::party::{kill_credit_share, PartyEffect, PartyRoster};
use crate::types::xp_to_next;
use crate::world::WORLD_SEED;
use crate::zones::populate_all_overworld;
use woc_content::{ability, class_def, PlayerClass, EASTBROOK};
use woc_protocol::{
    AbilityBarSlot, AuraSnapshot, CastSnapshot, EntityId, EntityKind, EntitySnapshot,
    EquipmentSnapshot, InteractAction, InvSlotSnapshot, PlayerIntent, PlayerProgress, SimEvent,
    TickSnapshot, WsServerMsg, DT, PROTOCOL_REV,
};

/// Max concurrent player entities on one Eastbrook realm (dev scaffold).
pub const MAX_REALM_PLAYERS: usize = 8;

/// Snapshot radius for other players, mobs, pets, and non-roll loot (yards).
pub const SNAPSHOT_AOI_RADIUS: f32 = 80.0;

pub struct Sim {
    pub tick: u64,
    pub seed: u32,
    pub rng: Rng,
    /// Authoritative actor store (typed sparse columns).
    pub world: World,
    /// Primary / local player id (offline host). `0` when the realm has no players yet.
    pub player_id: EntityId,
    pub events: Vec<SimEvent>,
    /// Per-player intents for the next tick.
    pub intents: HashMap<EntityId, PlayerIntent>,
    /// Party invite / membership roster for this realm.
    pub parties: PartyRoster,
    pub mail: crate::mail::Mailbox,
    pub market: crate::market::AuctionHouse,
    pub loot_rules: crate::social::LootRules,
    pub pvp: crate::pvp::PvpState,
}

impl Sim {
    /// Every player in the realm, dead or alive. See [`crate::ecs::living_player_ids`]
    /// for the alive-only variant.
    pub fn player_ids(&self) -> Vec<EntityId> {
        crate::ecs::player_ids(&self.world)
    }

    /// Continuous overworld (all zone bands) with no player. Online sticky realm.
    pub fn new_empty_eastbrook() -> Self {
        let seed = WORLD_SEED;
        let mut rng = Rng::new(seed);
        let mut world = World::new();
        populate_all_overworld(&mut world, &mut rng);

        Self {
            tick: 0,
            seed,
            rng,
            world,
            player_id: 0,
            events: Vec::new(),
            intents: HashMap::new(),
            parties: PartyRoster::new(),
            mail: crate::mail::Mailbox::new(),
            market: crate::market::AuctionHouse::new(),
            loot_rules: crate::social::LootRules::default(),
            pvp: crate::pvp::PvpState::default(),
        }
    }

    /// Start Eastbrook from content tables with one local player.
    pub fn new_eastbrook(player_name: &str, class: PlayerClass) -> Self {
        let mut sim = Self::new_empty_eastbrook();
        let _ = sim.spawn_player(player_name, class);
        sim
    }

    /// Backward-compatible Warrior Eastbrook spawn.
    pub fn new_combat_slice(player_name: &str) -> Self {
        Self::new_eastbrook(player_name, PlayerClass::Warrior)
    }

    /// Spawn a player into the existing realm without resetting NPCs/mobs.
    pub fn spawn_player(&mut self, name: &str, class: PlayerClass) -> Option<EntityId> {
        let players = self.player_count();
        if players >= MAX_REALM_PLAYERS {
            return None;
        }
        let id = self.world.next_id();
        let offset = players as f32 * 1.5;
        crate::ecs::spawn::create_player(
            &mut self.world,
            id,
            name,
            class,
            EASTBROOK.player_spawn_x + offset,
            EASTBROOK.player_spawn_z,
        );
        self.intents.insert(id, PlayerIntent::default());
        if self.player_id == 0 {
            self.player_id = id;
        }
        Some(id)
    }

    /// Spawn a player and apply durable progression (online persist path).
    pub fn spawn_player_with_state(
        &mut self,
        name: &str,
        class: PlayerClass,
        state: &crate::persist_state::PlayerPersistentState,
    ) -> Option<EntityId> {
        if let Some(ref did) = state.durable_id {
            if let Some(id) = self.resume_player(did) {
                return Some(id);
            }
        }
        let players = self.player_count();
        if players >= MAX_REALM_PLAYERS {
            return None;
        }
        if let Some(ref did) = state.durable_id {
            let dup = self
                .world
                .ids::<crate::ecs::components::Durable>()
                .into_iter()
                .any(|id| {
                    self.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Player)
                        && self
                            .world
                            .get::<crate::ecs::components::Durable>(id)
                            .and_then(|d| d.durable_id.as_deref())
                            == Some(did.as_str())
                });
            if dup {
                return None;
            }
        }
        let id = self.world.next_id();
        crate::persist_state::create_player_from_state(&mut self.world, id, name, class, state);
        let zone = self
            .world
            .get::<Identity>(id)
            .map(|i| i.zone_id.clone())
            .unwrap_or_default();
        if state.is_virgin()
            || (zone == "eastbrook" && state.pos_x.abs() < 0.01 && state.pos_z.abs() < 0.01)
        {
            let offset = players as f32 * 1.5;
            let x = EASTBROOK.player_spawn_x + offset;
            let z = EASTBROOK.player_spawn_z;
            let y = crate::ecs::spawn::ground_at(x, z);
            if let Some(t) = self.world.get_mut::<Transform>(id) {
                t.x = x;
                t.z = z;
                t.y = y;
            }
        }
        self.intents.insert(id, PlayerIntent::default());
        if self.player_id == 0 {
            self.player_id = id;
        }
        Some(id)
    }

    /// Export durable progression for a player (for disconnect autosave).
    pub fn export_player_state(
        &self,
        player_id: EntityId,
    ) -> Option<crate::persist_state::PlayerPersistentState> {
        crate::persist_state::export_player_state(&self.world, player_id)
    }

    /// Remove a player from the realm (disconnect). Does not recreate Eastbrook.
    pub fn despawn_player(&mut self, player_id: EntityId) {
        let _ = dismiss_pet(&mut self.world, player_id, &mut self.events);
        let _ = self.parties.on_despawn(player_id);
        self.world.despawn(player_id);
        self.intents.remove(&player_id);
        if self.player_id == player_id {
            self.player_id = self.player_ids().into_iter().next().unwrap_or(0);
        }
    }

    /// Park a player on disconnect: keep the entity and HP, drop intents.
    pub fn park_player(&mut self, player_id: EntityId) {
        if self.world.get::<ClassKit>(player_id).is_none() {
            return;
        }
        self.intents.remove(&player_id);
        if let Some(combat) = self.world.get_mut::<Combat>(player_id) {
            combat.auto_attack = false;
            combat.target = None;
            combat.cast = None;
        }
    }

    /// Resume a parked player by durable character id. Returns `None` if missing
    /// or still connected (has an intent slot).
    pub fn resume_player(&mut self, durable_id: &str) -> Option<EntityId> {
        let id = self.player_id_by_durable(durable_id)?;
        if self.intents.contains_key(&id) {
            return None;
        }
        self.intents.insert(id, PlayerIntent::default());
        if self.player_id == 0 {
            self.player_id = id;
        }
        Some(id)
    }

    fn player_id_by_durable(&self, durable_id: &str) -> Option<EntityId> {
        self.world
            .ids::<crate::ecs::components::Durable>()
            .into_iter()
            .find(|&id| {
                self.world.get::<ClassKit>(id).is_some()
                    && self
                        .world
                        .get::<crate::ecs::components::Durable>(id)
                        .and_then(|d| d.durable_id.as_deref())
                        == Some(durable_id)
            })
    }

    /// Party invite by target player name.
    pub fn party_invite(&mut self, player_id: EntityId, name: &str) -> Vec<WsServerMsg> {
        let effects = self.parties.invite(player_id, name, &self.world);
        map_party_effects(effects)
    }

    /// Accept a pending party invite.
    pub fn party_accept(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        let effects = self.parties.accept(player_id, &self.world);
        map_party_effects(effects)
    }

    /// Leave the current party (dissolves when size drops below 2).
    pub fn party_leave(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.leave(player_id))
    }

    /// Say / party chat.
    pub fn chat(&mut self, player_id: EntityId, channel: &str, text: &str) -> Vec<WsServerMsg> {
        map_chat_effects(handle_chat(
            &self.parties,
            &self.world,
            player_id,
            channel,
            text,
        ))
    }

    /// Current party member list for `player_id`, if any.
    pub fn party_members(&self, player_id: EntityId) -> Option<Vec<EntityId>> {
        self.parties.members_of(player_id)
    }

    pub fn player_count(&self) -> usize {
        self.player_ids().len()
    }

    /// Convenience: primary player copper (0 if none).
    pub fn copper(&self) -> u32 {
        self.world
            .get::<Progress>(self.player_id)
            .map(|p| p.copper)
            .unwrap_or(0)
    }

    /// Convenience: primary player XP (0 if none).
    pub fn player_xp(&self) -> u32 {
        self.world
            .get::<Progress>(self.player_id)
            .map(|p| p.xp)
            .unwrap_or(0)
    }

    /// Offline client: sticky auto-attack flag on the primary player's Combat column.
    pub fn player_auto_attack(&self) -> bool {
        self.world
            .get::<Combat>(self.player_id)
            .map(|c| c.auto_attack)
            .unwrap_or(false)
    }

    /// Offline client: current target from the Combat column.
    pub fn player_target(&self) -> Option<EntityId> {
        self.world
            .get::<Combat>(self.player_id)
            .and_then(|c| c.target)
    }

    /// Offline client: write Tab-cycle target into the Combat column.
    pub fn set_player_target(&mut self, target: Option<EntityId>) {
        if let Some(c) = self.world.get_mut::<Combat>(self.player_id) {
            c.target = target;
        }
    }

    pub fn interact(&mut self, target_id: EntityId, action: InteractAction) {
        woc_protocol::WorldHost::interact(self, self.player_id, target_id, action);
    }

    /// Tab-cycle living hostile mobs for the primary player.
    pub fn tab_target(&mut self) -> Option<EntityId> {
        let id = crate::targeting::tab_target(&self.world, self.player_id)?;
        if let Some(c) = self.world.get_mut::<Combat>(self.player_id) {
            c.target = Some(id);
        }
        Some(id)
    }

    /// Clear the primary player's current target and stop auto-attack (Esc).
    pub fn clear_target(&mut self) {
        if let Some(c) = self.world.get_mut::<Combat>(self.player_id) {
            c.target = None;
            c.auto_attack = false;
            c.cast = None;
        }
    }

    pub fn grant_item(
        &mut self,
        player_id: EntityId,
        item_id: &str,
        count: u32,
    ) -> Result<(), &'static str> {
        crate::inventory::grant_item(&mut self.world, player_id, item_id, count, &mut self.events)?;
        crate::quests::on_inventory_changed(&mut self.world, player_id, &mut self.events);
        Ok(())
    }

    /// Release a dead player's spirit: respawn at the Eastbrook graveyard.
    pub fn release_spirit(&mut self, player_id: EntityId) -> bool {
        crate::spirit::release_spirit(&mut self.world, player_id, &mut self.events)
    }

    /// Build a context bag for leaf modules (emit / lookup / mutate).
    pub fn context(&mut self) -> SimContext<'_> {
        SimContext {
            events: &mut self.events,
            world: &mut self.world,
            rng: &mut self.rng,
        }
    }

    /// Offline / single-intent tick: applies `intent` to the primary player.
    pub fn tick(&mut self, intent: PlayerIntent) -> (TickSnapshot, Vec<SimEvent>) {
        if self.player_id != 0 {
            self.intents.insert(self.player_id, intent);
        }
        self.tick_all()
    }

    /// Multi-player tick using the intent map.
    pub fn tick_all(&mut self) -> (TickSnapshot, Vec<SimEvent>) {
        self.events.clear();
        self.tick += 1;

        let player_ids = self.player_ids();
        for &pid in &player_ids {
            refresh_daily_quests(&mut self.world, pid, self.tick);
        }

        // Phase 1: apply intents + motion
        for &pid in &player_ids {
            let intent = self.intents.get(&pid).copied().unwrap_or_default();
            let alive = self
                .world
                .get::<Health>(pid)
                .map(|h| h.alive)
                .unwrap_or(false);
            if !alive {
                continue;
            }
            if intent.clear_target {
                if let Some(c) = self.world.get_mut::<Combat>(pid) {
                    c.target = None;
                    c.auto_attack = false;
                    c.cast = None;
                }
            }
            let effect = step_player_motion(&mut self.world, pid, &intent);
            if intent.fly_toggle {
                let flying = self
                    .world
                    .get::<Motion>(pid)
                    .map(|m| m.flying)
                    .unwrap_or(false);
                self.events.push(woc_protocol::SimEvent::Toast {
                    message: if flying {
                        "Travel flight engaged (Space up · Ctrl down · V land).".into()
                    } else {
                        "Travel flight disengaged.".into()
                    },
                });
            }
            if let Some(effect) = effect {
                if effect.fall_damage > 0.0 {
                    let mut died = false;
                    if let Some(h) = self.world.get_mut::<Health>(pid) {
                        h.hp = (h.hp - effect.fall_damage).max(0.0);
                        died = h.hp <= 0.0;
                    }
                    self.events.push(woc_protocol::SimEvent::Toast {
                        message: format!("Falling deals {} damage.", effect.fall_damage as i32),
                    });
                    if died {
                        crate::death::on_player_death_check(&mut self.world, &mut self.events);
                    }
                }
            }
            if let Some(tid) = intent.target_id {
                if let Some(c) = self.world.get_mut::<Combat>(pid) {
                    c.target = Some(tid);
                }
            }
            if intent.attack {
                let need_acquire = self
                    .world
                    .get::<Combat>(pid)
                    .map(|c| c.target.is_none())
                    .unwrap_or(false);
                let acquired = if need_acquire {
                    nearest_mob(&self.world, pid, 30.0)
                } else {
                    None
                };
                if let Some(c) = self.world.get_mut::<Combat>(pid) {
                    c.auto_attack = true;
                    if c.target.is_none() {
                        c.target = acquired;
                    }
                }
            }
            credit_explore(&mut self.world, pid, &mut self.events);
        }
        // Phase 2: player_combat
        for &pid in &player_ids {
            let ability = self.intents.get(&pid).and_then(|i| i.ability);
            update_player_combat(
                pid,
                &mut self.world,
                ability,
                &mut self.rng,
                &mut self.events,
            );
        }

        // Phase 3: pet_ai
        let _dropped = tick_pets(&mut self.world, &mut self.events);
        tick_escorts(&mut self.world, &mut self.events);

        // Phase 4: mob_ai_combat (focus nearest living player)
        let mob_ids: Vec<EntityId> = self
            .world
            .ids::<LootTable>()
            .into_iter()
            .filter(|&id| {
                self.world
                    .get::<Health>(id)
                    .map(|h| h.alive)
                    .unwrap_or(false)
            })
            .collect();
        for mid in &mob_ids {
            if let Some(pid) = nearest_alive_player(&self.world, *mid, 40.0) {
                update_mob_ai(&mut self.world, *mid, pid);
            }
        }
        for mid in &mob_ids {
            let focus = self
                .world
                .get::<Combat>(*mid)
                .and_then(|c| c.target)
                .or_else(|| nearest_alive_player(&self.world, *mid, 40.0));
            if let Some(pid) = focus {
                update_mob_combat(*mid, pid, &mut self.world, &mut self.events);
            }
        }

        // Phase 5: aura_decay
        tick_auras(&mut self.world, &mut self.events);
        tick_mob_respawns(&mut self.world, DT);

        // Phase 6: kill_rewards → killer (+ party share)
        let rewards = collect_pending_mob_kills(&self.events, &self.world);
        for reward in rewards {
            let mut recipients = vec![reward.killer];
            for mate in kill_credit_share(&self.parties, &self.world, reward.killer) {
                if !recipients.contains(&mate) {
                    recipients.push(mate);
                }
            }
            for rid in recipients {
                if self.world.get::<Identity>(rid).map(|i| i.kind) == Some(EntityKind::Player) {
                    grant_xp(&mut self.world, rid, reward.xp, &mut self.events);
                    if let Some(ref tid) = reward.template_id {
                        on_mob_killed(&mut self.world, rid, tid, &mut self.events);
                        crate::worldboss::on_boss_killed(
                            &mut self.world,
                            rid,
                            tid,
                            &mut self.events,
                        );
                    }
                }
            }
            let loot_before: HashSet<EntityId> = self.world.ids::<LootPile>().into_iter().collect();
            let _first = spawn_mob_loot(
                &mut self.world,
                &mut self.rng,
                reward.template_id.as_deref(),
                reward.x,
                reward.z,
            );
            let killer_inst = self
                .world
                .get::<InstanceAt>(reward.killer)
                .and_then(|i| i.instance_id.clone());
            for loot_id in self
                .world
                .ids::<LootPile>()
                .into_iter()
                .filter(|id| !loot_before.contains(id))
            {
                if let Some(ref inst) = killer_inst {
                    if let Some(loot) = self.world.get_mut::<InstanceAt>(loot_id) {
                        loot.instance_id = Some(inst.clone());
                    }
                }
                self.loot_rules.maybe_start_party_roll(
                    &self.parties,
                    &self.world,
                    reward.killer,
                    loot_id,
                    &mut self.events,
                );
            }
        }
        // ws-death: finalize player deaths (corpse + PlayerDied) after kill rewards
        crate::death::on_player_death_check(&mut self.world, &mut self.events);

        // Phase 7: pvp_and_market
        crate::pvp::tick_pvp(&mut self.pvp, &mut self.world, &mut self.events);
        self.market
            .tick_expire(self.tick, &mut self.world, &mut self.mail);

        // Phase 8: loot_pickup
        for &pid in &player_ids {
            try_pickup_loot(pid, &mut self.world, &mut self.events, &self.loot_rules);
        }

        // Phase 9: profession_casts
        crate::professions::tick_profession_casts(
            &mut self.world,
            self.tick,
            &mut self.rng,
            &mut self.events,
        );

        // Phase 10: build_snapshot
        let viewer = if self.player_id != 0 {
            self.player_id
        } else {
            player_ids.first().copied().unwrap_or(0)
        };
        let snapshot = self.snapshot_for_player(viewer);
        let events = self.events.clone();
        (snapshot, events)
    }

    pub fn snapshot(&self) -> TickSnapshot {
        self.snapshot_for_player(self.player_id)
    }

    pub fn snapshot_for_player(&self, player_id: EntityId) -> TickSnapshot {
        let world = &self.world;
        let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
        let target_id = world.get::<Combat>(player_id).and_then(|c| c.target);
        let ability_cd = world
            .get::<Combat>(player_id)
            .map(|c| c.ability_cd)
            .unwrap_or(0.0);
        let primary_ability = world
            .get::<ClassKit>(player_id)
            .and_then(|k| k.primary_ability.clone());
        let ability_name = primary_ability
            .as_deref()
            .and_then(ability)
            .map(|a| a.name.to_string())
            .unwrap_or_default();
        let cd_max = primary_ability
            .as_deref()
            .and_then(ability)
            .map(|a| a.cooldown)
            .unwrap_or(3.0);
        let class_id = world
            .get::<ClassKit>(player_id)
            .and_then(|k| k.class_id)
            .map(|c| c.as_str().to_string())
            .unwrap_or_default();
        let resource_type = world
            .get::<ClassKit>(player_id)
            .and_then(|k| k.class_id)
            .map(|c| class_def(c).resource_type)
            .map(|rt| match rt {
                woc_content::ResourceType::Rage => "rage",
                woc_content::ResourceType::Mana => "mana",
                woc_content::ResourceType::Energy => "energy",
            })
            .unwrap_or("")
            .to_string();

        let inventory = world
            .get::<Bags>(player_id)
            .map(|bags| {
                bags.inventory
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        s.as_ref().map(|st| InvSlotSnapshot {
                            slot: i as u8,
                            item_id: st.item_id.clone(),
                            count: st.count,
                            durability: st.durability,
                            enchant_id: st.enchant_id.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let equipment = world
            .get::<Bags>(player_id)
            .map(|bags| EquipmentSnapshot {
                main_hand: bags.equipment.main_hand.clone(),
                off_hand: bags.equipment.off_hand.clone(),
                head: bags.equipment.head.clone(),
                chest: bags.equipment.chest.clone(),
                legs: bags.equipment.legs.clone(),
                feet: bags.equipment.feet.clone(),
                neck: bags.equipment.neck.clone(),
                finger: bags.equipment.finger.clone(),
                finger2: bags.equipment.finger2.clone(),
                main_hand_enchant: bags.equipment_enchants.main_hand.clone(),
                main_hand_durability: bags
                    .equipment
                    .main_hand
                    .as_ref()
                    .and(bags.equipment_wear.main_hand),
                off_hand_durability: bags
                    .equipment
                    .off_hand
                    .as_ref()
                    .and(bags.equipment_wear.off_hand),
                head_durability: bags.equipment.head.as_ref().and(bags.equipment_wear.head),
                chest_durability: bags.equipment.chest.as_ref().and(bags.equipment_wear.chest),
                legs_durability: bags.equipment.legs.as_ref().and(bags.equipment_wear.legs),
                feet_durability: bags.equipment.feet.as_ref().and(bags.equipment_wear.feet),
            })
            .unwrap_or_default();

        let quest_log = quest_log_entries(world, player_id);

        let open_vendor = vendor_snapshot(world, player_id);
        let open_npc = npc_session_snapshot(world, player_id);
        let hearth_ready_tick = world
            .get::<Hearth>(player_id)
            .map(|h| h.ready_tick)
            .unwrap_or(0);
        let hearth_zone_id = world
            .get::<Hearth>(player_id)
            .map(|h| h.zone_id.clone())
            .unwrap_or_default();

        let auras = world
            .get::<Auras>(player_id)
            .map(|a| {
                a.auras
                    .iter()
                    .map(|aura| AuraSnapshot {
                        id: aura.id.clone(),
                        remaining: aura.remaining.max(0.0),
                        stacks: aura.stacks,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cast = world.get::<Combat>(player_id).and_then(|c| {
            c.cast.as_ref().map(|cast_state| CastSnapshot {
                ability_id: cast_state.ability_id.clone(),
                progress: if cast_state.duration > 0.0 {
                    (cast_state.elapsed / cast_state.duration).clamp(0.0, 1.0)
                } else {
                    1.0
                },
            })
        });

        let gcd = world.get::<Combat>(player_id).map(|c| c.gcd).unwrap_or(0.0);
        let casting = world
            .get::<Combat>(player_id)
            .map(|c| c.cast.is_some())
            .unwrap_or(false);
        let auto_attack = world
            .get::<Combat>(player_id)
            .map(|c| c.auto_attack)
            .unwrap_or(false);
        let ability_bar = build_ability_bar(world, player_id, gcd, casting);

        let viewer_instance = world
            .get::<InstanceAt>(player_id)
            .and_then(|i| i.instance_id.clone());
        let entities = world
            .live_ids()
            .filter(|&id| self.snapshot_visible(player_id, viewer_instance.as_deref(), id))
            .filter_map(|id| entity_snapshot(world, id))
            .collect();

        TickSnapshot {
            tick: self.tick,
            player_id,
            entities,
            progress: PlayerProgress {
                xp: world.get::<Progress>(player_id).map(|p| p.xp).unwrap_or(0),
                xp_to_level: xp_to_next(level),
                level,
                copper: world
                    .get::<Progress>(player_id)
                    .map(|p| p.copper)
                    .unwrap_or(0),
                bag_item: None,
                class_id,
                resource_type,
            },
            target_id,
            ability_ready: ability_cd <= 0.0 && gcd <= 0.0 && !casting,
            ability_cooldown: if cd_max > 0.0 {
                ability_cd / cd_max
            } else {
                0.0
            },
            protocol_rev: PROTOCOL_REV,
            inventory,
            equipment,
            quest_log,
            open_vendor,
            open_npc,
            ability_name,
            auras,
            cast,
            ability_bar,
            gcd,
            auto_attack,
            is_dead: world
                .get::<Health>(player_id)
                .map(|h| !h.alive)
                .unwrap_or(false),
            party_id: self.parties.party_id(player_id),
            party_leader_id: None,
            party_kind: String::new(),
            party_members: Vec::new(),
            pending_invite_from: String::new(),
            ready_check: None,
            zone_id: world
                .get::<Identity>(player_id)
                .map(|i| i.zone_id.clone())
                .unwrap_or_else(|| "eastbrook".into()),
            hearth_ready_tick,
            hearth_zone_id,
            talent_points: world
                .get::<Progress>(player_id)
                .map(|p| p.talent_points)
                .unwrap_or(0),
            talents: world
                .get::<Progress>(player_id)
                .map(|p| {
                    p.talents
                        .iter()
                        .map(|(id, rank)| woc_protocol::TalentRankSnapshot {
                            talent_id: id.clone(),
                            rank: *rank,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            bank: world
                .get::<Bank>(player_id)
                .map(|bank| {
                    bank.bank
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| {
                            s.as_ref().map(|st| InvSlotSnapshot {
                                slot: i as u8,
                                item_id: st.item_id.clone(),
                                count: st.count,
                                durability: st.durability,
                                enchant_id: st.enchant_id.clone(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            mail: self.mail.snapshot_for_entity(player_id, world),
            market: self.market.snapshot_for(player_id, world),
            honor: world
                .get::<Progress>(player_id)
                .map(|p| p.honor)
                .unwrap_or(0),
            pvp_flagged: world
                .get::<Progress>(player_id)
                .map(|p| p.pvp_flagged)
                .unwrap_or(false),
            professions: world
                .get::<Progress>(player_id)
                .map(|p| {
                    p.professions
                        .iter()
                        .map(|(id, skill)| woc_protocol::ProfessionSkillSnapshot {
                            id: id.clone(),
                            skill: *skill,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            loot_mode: self.parties.loot_mode(player_id),
            pending_loot: self.loot_rules.snapshot_for(player_id),
            bank_copper: world
                .get::<Bank>(player_id)
                .map(|b| b.bank_copper)
                .unwrap_or(0),
            combo_points: world
                .get::<ClassKit>(player_id)
                .map(|k| k.combo_points)
                .unwrap_or(0),
            stealthed: world
                .get::<ClassKit>(player_id)
                .is_some_and(|k| k.stealthed),
            stance_id: world
                .get::<ClassKit>(player_id)
                .and_then(|k| k.stance_id.clone())
                .unwrap_or_default(),
            absorb: crate::combat::remaining_absorb(world, player_id),
            attack_power: world
                .get::<Combat>(player_id)
                .map(|c| c.attack_damage)
                .unwrap_or(0.0),
            armor: world
                .get::<Combat>(player_id)
                .map(|c| c.armor)
                .unwrap_or(0.0),
            spell_power: world
                .get::<Combat>(player_id)
                .map(|c| c.spell_power)
                .unwrap_or(0.0),
        }
    }

    fn snapshot_visible(
        &self,
        viewer: EntityId,
        viewer_instance: Option<&str>,
        id: EntityId,
    ) -> bool {
        if !snapshot_includes_entity(&self.world, viewer_instance, id) {
            return false;
        }
        if id == viewer {
            return true;
        }
        if self
            .parties
            .members_of(viewer)
            .is_some_and(|m| m.contains(&id))
        {
            return true;
        }
        if self.world.get::<Combat>(viewer).and_then(|c| c.target) == Some(id) {
            return true;
        }
        let Some(identity) = self.world.get::<Identity>(id) else {
            return false;
        };
        let viewer_zone = self
            .world
            .get::<Identity>(viewer)
            .map(|i| i.zone_id.as_str())
            .unwrap_or("");
        if identity.kind == EntityKind::Npc && identity.zone_id == viewer_zone {
            return true;
        }
        if self
            .loot_rules
            .pending
            .iter()
            .any(|p| p.loot_id == id && p.eligible.contains(&viewer))
        {
            return true;
        }
        crate::ecs::components::dist2d(&self.world, viewer, id)
            .is_some_and(|d| d <= SNAPSHOT_AOI_RADIUS)
    }
}

fn entity_snapshot(world: &World, id: EntityId) -> Option<EntitySnapshot> {
    let identity = world.get::<Identity>(id)?;
    let t = world.get::<Transform>(id)?;
    let health = world.get::<Health>(id);
    let motion = world.get::<Motion>(id);
    let kit = world.get::<ClassKit>(id);
    Some(EntitySnapshot {
        id,
        kind: identity.kind,
        x: t.x,
        y: t.y,
        z: t.z,
        yaw: t.yaw,
        hp: health.map(|h| h.hp).unwrap_or(0.0),
        hp_max: health.map(|h| h.hp_max).unwrap_or(0.0),
        level: health.map(|h| h.level).unwrap_or(1),
        name: identity.name.clone(),
        resource: kit.map(|k| k.resource).unwrap_or(0.0),
        resource_max: kit.map(|k| k.resource_max).unwrap_or(0.0),
        alive: health.map(|h| h.alive).unwrap_or(true),
        template_id: identity.template_id.clone(),
        on_ground: motion.map(|m| m.on_ground).unwrap_or(true),
        flying: motion.map(|m| m.flying).unwrap_or(false),
        swimming: crate::player_motion::is_swimming_at(t.x, t.y, t.z),
    })
}

fn snapshot_includes_entity(world: &World, viewer_instance: Option<&str>, id: EntityId) -> bool {
    let Some(identity) = world.get::<Identity>(id) else {
        return false;
    };
    let alive = world.get::<Health>(id).map(|h| h.alive).unwrap_or(true);
    if !alive && identity.kind != EntityKind::Mob && identity.kind != EntityKind::Npc {
        return false;
    }
    let entity_instance = world
        .get::<InstanceAt>(id)
        .and_then(|i| i.instance_id.as_deref());
    match (viewer_instance, entity_instance) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        (Some(_), None) => identity.kind == EntityKind::Player,
        (None, Some(_)) => identity.kind == EntityKind::Player,
    }
}

fn build_ability_bar(
    world: &World,
    player_id: EntityId,
    gcd: f32,
    casting: bool,
) -> Vec<AbilityBarSlot> {
    let Some(kit) = world.get::<ClassKit>(player_id) else {
        return Vec::new();
    };
    let Some(class) = kit.class_id else {
        return Vec::new();
    };
    let ability_cd = world
        .get::<Combat>(player_id)
        .map(|c| c.ability_cd)
        .unwrap_or(0.0);
    let def = class_def(class);
    def.kit
        .iter()
        .map(|entry| {
            let abil = ability(entry.ability_id);
            let name = abil.map(|a| a.name.to_string()).unwrap_or_else(|| {
                entry
                    .ability_id
                    .replace('_', " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            let known = kit.known_abilities.iter().any(|id| id == entry.ability_id);
            let cd = kit
                .ability_cds
                .get(entry.ability_id)
                .copied()
                .unwrap_or(0.0)
                .max(if entry.slot == 1 { ability_cd } else { 0.0 });
            let cost = abil.map(|a| a.cost).unwrap_or(0.0);
            let affordable = kit.resource + 1e-3 >= cost;
            let ready = known && cd <= 0.0 && gcd <= 0.0 && !casting && affordable;
            AbilityBarSlot {
                slot: entry.slot,
                ability_id: entry.ability_id.to_string(),
                name,
                known,
                ready,
                cooldown: cd.max(0.0),
            }
        })
        .collect()
}

fn map_party_effects(effects: Vec<PartyEffect>) -> Vec<WsServerMsg> {
    effects
        .into_iter()
        .map(|e| match e {
            PartyEffect::Update { members } => WsServerMsg::PartyUpdate { members },
            // Prefer Chat toasts over Error so the client does not flip NetStatus.
            PartyEffect::Error { message } | PartyEffect::Notice { message } => WsServerMsg::Chat {
                channel: "system".into(),
                from: "Party".into(),
                text: message,
            },
        })
        .collect()
}

fn map_chat_effects(effects: Vec<ChatEffect>) -> Vec<WsServerMsg> {
    effects
        .into_iter()
        .map(|e| match e {
            ChatEffect::Message {
                channel,
                from,
                text,
            } => WsServerMsg::Chat {
                channel,
                from,
                text,
            },
            ChatEffect::Error { message } => WsServerMsg::Chat {
                channel: "system".into(),
                from: "Chat".into(),
                text: message,
            },
        })
        .collect()
}

fn nearest_mob(world: &World, from: EntityId, max_range: f32) -> Option<EntityId> {
    let from_t = world.get::<Transform>(from)?;
    let mut best: Option<(EntityId, f32)> = None;
    for id in world.ids::<LootTable>() {
        if world.get::<Owner>(id).is_some() || world.get::<ClassKit>(id).is_some() {
            continue;
        }
        let Some(h) = world.get::<Health>(id) else {
            continue;
        };
        if !h.alive {
            continue;
        }
        let Some(t) = world.get::<Transform>(id) else {
            continue;
        };
        let dx = t.x - from_t.x;
        let dz = t.z - from_t.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d > max_range {
            continue;
        }
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((id, d));
        }
    }
    best.map(|(id, _)| id)
}

fn nearest_alive_player(world: &World, from: EntityId, max_range: f32) -> Option<EntityId> {
    let from_t = world.get::<Transform>(from)?;
    let mut best: Option<(EntityId, f32)> = None;
    for id in world.ids::<ClassKit>() {
        let Some(h) = world.get::<Health>(id) else {
            continue;
        };
        if !h.alive {
            continue;
        }
        let Some(t) = world.get::<Transform>(id) else {
            continue;
        };
        let dx = t.x - from_t.x;
        let dz = t.z - from_t.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d > max_range {
            continue;
        }
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((id, d));
        }
    }
    best.map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{tick_phase_fingerprint, TICK_PHASES};
    use crate::ecs::components::{
        Bags, Bank, ClassKit, Health, LootPile, Owner, QuestLog, QuestState, Threat, Transform,
    };
    use crate::ecs::spawn;
    use woc_protocol::{AbilitySlot, InteractAction, WorldHost};

    fn kind_count(sim: &Sim, kind: EntityKind) -> usize {
        sim.world
            .live_ids()
            .filter(|&id| sim.world.get::<Identity>(id).map(|i| i.kind) == Some(kind))
            .count()
    }

    fn find_template(sim: &Sim, template: &str) -> Option<EntityId> {
        sim.world.live_ids().find(|&id| {
            sim.world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some(template)
        })
    }

    fn place_player_at(sim: &mut Sim, x: f32, z: f32) {
        let y = crate::ecs::spawn::ground_at(x, z);
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = x;
            t.z = z;
            t.y = y;
        }
    }

    #[test]
    fn catalog_sparsity_by_kind() {
        let sim = Sim::new_eastbrook("Sparse", woc_content::PlayerClass::Warrior);
        for id in sim.world.live_ids() {
            let kind = sim.world.get::<Identity>(id).unwrap().kind;
            match kind {
                EntityKind::Loot => {
                    assert!(sim.world.get::<Bags>(id).is_none());
                    assert!(sim.world.get::<Bank>(id).is_none());
                    assert!(sim.world.get::<LootPile>(id).is_some());
                }
                EntityKind::Npc => {
                    assert!(sim.world.get::<Bags>(id).is_none());
                    assert!(sim.world.get::<Combat>(id).is_none());
                }
                EntityKind::Mob => {
                    assert!(sim.world.get::<Bags>(id).is_none());
                    assert!(sim.world.get::<Threat>(id).is_some());
                }
                EntityKind::Player => {
                    assert!(sim.world.get::<Bags>(id).is_some());
                    assert!(sim.world.get::<ClassKit>(id).is_some());
                }
                EntityKind::Pet => {
                    assert!(sim.world.get::<Owner>(id).is_some());
                    assert!(sim.world.get::<Bags>(id).is_none());
                }
            }
        }
    }

    #[test]
    fn snapshot_entity_ids_match_live_ids() {
        let sim = Sim::new_eastbrook("Snap", PlayerClass::Warrior);
        let snap = sim.snapshot_for_player(sim.player_id);
        let viewer_instance = sim
            .world
            .get::<InstanceAt>(sim.player_id)
            .and_then(|i| i.instance_id.clone());
        let expected: Vec<EntityId> = sim
            .world
            .live_ids()
            .filter(|&id| sim.snapshot_visible(sim.player_id, viewer_instance.as_deref(), id))
            .filter(|&id| entity_snapshot(&sim.world, id).is_some())
            .collect();
        let snap_ids: Vec<EntityId> = snap.entities.iter().map(|e| e.id).collect();
        assert_eq!(snap_ids, expected);
        assert_eq!(snap.entities.len(), expected.len());
    }

    #[test]
    fn tick_phase_order_fingerprint_locked() {
        assert_eq!(TICK_PHASES.len(), 10);
        assert_eq!(TICK_PHASES[0], "apply_intents_motion");
        assert_eq!(TICK_PHASES[2], "pet_ai");
        assert_eq!(TICK_PHASES[6], "pvp_and_market");
        assert_eq!(TICK_PHASES[8], "profession_casts");
        assert_eq!(TICK_PHASES[9], "build_snapshot");
        assert_eq!(tick_phase_fingerprint(), 3214741777866168171u64);
    }

    #[test]
    fn sim_context_emit_and_lookup() {
        let mut sim = Sim::new_eastbrook("Ctx", PlayerClass::Warrior);
        {
            let mut ctx = sim.context();
            ctx.emit(SimEvent::Toast {
                message: "hi".into(),
            });
            assert_eq!(ctx.player_ids().len(), 1);
            assert_eq!(ctx.living_player_ids().len(), 1);
        }
        assert_eq!(sim.events.len(), 1);
    }

    #[test]
    fn player_ids_keeps_the_dead_and_living_player_ids_does_not() {
        let mut sim = Sim::new_eastbrook("Ghost", PlayerClass::Warrior);
        let pid = sim.player_id;
        sim.world.get_mut::<Health>(pid).unwrap().alive = false;

        assert_eq!(sim.player_ids(), vec![pid]);
        let ctx = sim.context();
        assert_eq!(ctx.player_ids(), vec![pid]);
        assert!(ctx.living_player_ids().is_empty());
    }

    #[test]
    fn player_ids_excludes_npcs_mobs_and_loot_in_a_populated_realm() {
        let mut sim = Sim::new_eastbrook("Solo", PlayerClass::Warrior);
        assert!(kind_count(&sim, EntityKind::Npc) >= 3);
        assert!(kind_count(&sim, EntityKind::Mob) >= 3);
        let loot_id = sim.world.next_id();
        crate::ecs::spawn::create_loot(&mut sim.world, loot_id, 0.0, 0.0, 5, None);

        assert_eq!(sim.player_ids(), vec![sim.player_id]);
    }

    #[test]
    fn loot_has_no_bags_column() {
        let mut sim = Sim::new_eastbrook("LootCol", PlayerClass::Warrior);
        let id = sim.world.next_id();
        crate::ecs::spawn::create_loot(&mut sim.world, id, 0.0, 0.0, 5, None);
        assert!(sim.world.get::<LootPile>(id).is_some());
        assert!(sim.world.get::<Bags>(id).is_none());
        assert!(sim.world.get::<Bank>(id).is_none());
        assert!(sim.world.get::<ClassKit>(id).is_none());
        let player_id = sim.player_id;
        assert!(sim.world.get::<Bags>(player_id).is_some());
        assert!(sim.world.get::<LootPile>(player_id).is_none());
    }

    #[test]
    fn eastbrook_spawns_npcs_and_mobs_from_content() {
        let sim = Sim::new_eastbrook("Tester", PlayerClass::Warrior);
        assert!(kind_count(&sim, EntityKind::Npc) >= 3);
        assert!(kind_count(&sim, EntityKind::Mob) >= 4);
    }

    #[test]
    fn sticky_spawn_does_not_reset_npcs() {
        let mut sim = Sim::new_empty_eastbrook();
        let npc_before = kind_count(&sim, EntityKind::Npc);
        let mob_before = kind_count(&sim, EntityKind::Mob);
        let a = sim.spawn_player("A", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("B", PlayerClass::Mage).unwrap();
        assert_ne!(a, b);
        assert_eq!(sim.player_count(), 2);
        assert_eq!(kind_count(&sim, EntityKind::Npc), npc_before);
        assert_eq!(kind_count(&sim, EntityKind::Mob), mob_before);
        sim.despawn_player(a);
        assert_eq!(sim.player_count(), 1);
        assert_eq!(kind_count(&sim, EntityKind::Npc), npc_before);
        assert!(sim.world.contains(b));
    }

    #[test]
    fn all_nine_classes_spawn() {
        for class in PlayerClass::ALL {
            let sim = Sim::new_eastbrook("C", class);
            let h = sim.world.get::<Health>(sim.player_id).unwrap();
            assert!(h.alive);
            assert!(h.hp_max > 0.0);
            assert!(sim
                .world
                .get::<Bags>(sim.player_id)
                .unwrap()
                .equipment
                .main_hand
                .is_some());
        }
    }

    #[test]
    fn loot_goes_into_backpack() {
        let mut sim = Sim::new_eastbrook("Looter", PlayerClass::Warrior);
        assert!(sim.grant_item(sim.player_id, "wolf_fang", 1).is_ok());
        let snap = sim.snapshot_for_player(sim.player_id);
        assert!(snap
            .inventory
            .iter()
            .any(|s| s.item_id == "wolf_fang" && s.count == 1));
    }

    #[test]
    fn pvp_honor_survives_phase5_loot_apply() {
        let mut sim = Sim::new_eastbrook("Winner", PlayerClass::Warrior);
        let winner = sim.player_id;
        let loser = sim.spawn_player("Loser", PlayerClass::Mage).unwrap();
        place_player_at(&mut sim, 0.0, 0.0);
        {
            let y = crate::ecs::spawn::ground_at(1.0, 0.0);
            if let Some(t) = sim.world.get_mut::<Transform>(loser) {
                t.x = 1.0;
                t.z = 0.0;
                t.y = y;
            }
            if let Some(h) = sim.world.get_mut::<Health>(loser) {
                h.hp = 1.0;
            }
        }

        let mut duel_events = Vec::new();
        crate::pvp::challenge_duel(&mut sim.pvp, &sim.world, winner, loser).unwrap();
        crate::pvp::accept_duel(&mut sim.pvp, &sim.world, loser, winner, &mut duel_events).unwrap();

        let (wx, wz) = {
            let t = sim.world.get::<Transform>(winner).unwrap();
            (t.x, t.z)
        };
        let loot_id = sim.world.next_id();
        crate::ecs::spawn::create_loot(&mut sim.world, loot_id, wx, wz, 7, None);

        assert_eq!(sim.world.get::<Progress>(winner).unwrap().honor, 0);

        let (_snap, events) = sim.tick(PlayerIntent::default());

        assert_eq!(
            sim.world.get::<Progress>(winner).unwrap().honor,
            crate::pvp::HONOR_PER_KILL
        );
        assert_eq!(sim.copper(), 7);
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::HonorGained {
                player,
                amount: crate::pvp::HONOR_PER_KILL
            } if *player == winner
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Loot {
                player,
                copper: 7,
                ..
            } if *player == winner
        )));
    }

    #[test]
    fn wolf_quest_accept_kill_turnin() {
        let mut sim = Sim::new_eastbrook("Q", PlayerClass::Warrior);
        let giver = find_template(&sim, "captain_alden").unwrap();
        let crier = find_template(&sim, "town_crier").unwrap();
        let (cx, cz) = {
            let t = sim.world.get::<Transform>(crier).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, cx, cz);
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        let (gx, gz) = {
            let t = sim.world.get::<Transform>(giver).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, gx, gz);
        sim.interact(giver, InteractAction::Talk);
        sim.interact(
            giver,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
                reward_choice: None,
            },
        );
        sim.interact(
            giver,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );

        let wolf_ids: Vec<EntityId> = sim
            .world
            .live_ids()
            .filter(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some("young_wolf")
                    && sim
                        .world
                        .get::<Health>(id)
                        .map(|h| h.alive)
                        .unwrap_or(false)
            })
            .take(3)
            .collect();
        assert_eq!(wolf_ids.len(), 3);

        for wid in wolf_ids {
            let (wx, wz) = {
                let t = sim.world.get::<Transform>(wid).unwrap();
                (t.x, t.z)
            };
            place_player_at(&mut sim, wx, wz);
            if let Some(kit) = sim.world.get_mut::<ClassKit>(sim.player_id) {
                kit.resource = 100.0;
            }
            if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
                c.target = Some(wid);
                c.auto_attack = true;
            }
            let intent = PlayerIntent {
                attack: true,
                ability: Some(AbilitySlot::Primary),
                target_id: Some(wid),
                ..Default::default()
            };
            for _ in 0..200 {
                let (_s, ev) = sim.tick(intent);
                if ev
                    .iter()
                    .any(|e| matches!(e, SimEvent::Kill { victim, .. } if *victim == wid))
                {
                    break;
                }
            }
        }

        let ready = sim
            .world
            .get::<QuestLog>(sim.player_id)
            .unwrap()
            .quest_log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == QuestState::Ready);
        assert!(ready, "quest should be ready after 3 kills");

        let (gx, gz) = {
            let t = sim.world.get::<Transform>(giver).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, gx, gz);
        sim.interact(
            giver,
            InteractAction::TurnInQuest {
                quest_id: "wolves_at_the_gate".into(),
                reward_choice: None,
            },
        );
        let log = WorldHost::snapshot_for(&sim, sim.player_id).quest_log;
        assert!(log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == "completed"));
    }

    #[test]
    fn talk_to_trainer_opens_npc_session_without_vendor() {
        let mut sim = Sim::new_eastbrook("T", PlayerClass::Warrior);
        let wren = find_template(&sim, "herbalist_wren").expect("herbalist_wren");
        let (x, z) = {
            let t = sim.world.get::<Transform>(wren).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        sim.interact(wren, InteractAction::Talk);
        let snap = sim.snapshot_for_player(sim.player_id);
        assert!(snap.open_vendor.is_none());
        let session = snap.open_npc.expect("session");
        assert_eq!(session.npc_id, wren);
        assert!(session.train_professions.contains(&"herbalism".to_string()));
        assert!(!session.can_repair);
    }

    #[test]
    fn train_mining_requires_smith() {
        let mut sim = Sim::new_eastbrook("P", PlayerClass::Warrior);
        sim.interact(
            sim.player_id,
            InteractAction::TrainProfession {
                id: "mining".into(),
            },
        );
        assert!(sim
            .world
            .get::<Progress>(sim.player_id)
            .unwrap()
            .professions
            .get("mining")
            .is_none());

        let smith = find_template(&sim, "smith_brann").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(smith).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        sim.interact(smith, InteractAction::Talk);
        sim.interact(
            smith,
            InteractAction::TrainProfession {
                id: "mining".into(),
            },
        );
        assert_eq!(
            sim.world
                .get::<Progress>(sim.player_id)
                .unwrap()
                .professions
                .get("mining")
                .copied(),
            Some(1)
        );

        sim.interact(
            smith,
            InteractAction::TrainProfession {
                id: "herbalism".into(),
            },
        );
        assert!(sim
            .world
            .get::<Progress>(sim.player_id)
            .unwrap()
            .professions
            .get("herbalism")
            .is_none());
    }

    #[test]
    fn repair_all_at_smith_restores_gear() {
        let mut sim = Sim::new_eastbrook("R", PlayerClass::Warrior);
        let smith = find_template(&sim, "smith_brann").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(smith).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
            bags.equipment_wear.main_hand = Some(1);
            bags.equipment_wear.chest = Some(1);
        }
        if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
            p.copper = 10_000;
        }
        let cost = {
            let sword = 40 - 1;
            let tunic = 30 - 1;
            sword + tunic
        };
        sim.interact(smith, InteractAction::Talk);
        let session = sim.snapshot_for_player(sim.player_id).open_npc.unwrap();
        assert!(session.can_repair);
        assert_eq!(session.repair_cost, cost);
        let copper_before = sim.copper();
        sim.interact(smith, InteractAction::RepairAll);
        assert_eq!(sim.copper(), copper_before - cost);
        let wear = &sim.world.get::<Bags>(sim.player_id).unwrap().equipment_wear;
        assert_eq!(wear.main_hand, Some(40));
        assert_eq!(wear.chest, Some(30));
    }

    #[test]
    fn repair_refuses_without_copper() {
        let mut sim = Sim::new_eastbrook("R", PlayerClass::Warrior);
        let smith = find_template(&sim, "smith_brann").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(smith).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
            bags.equipment_wear.main_hand = Some(0);
        }
        if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
            p.copper = 0;
        }
        sim.interact(smith, InteractAction::Talk);
        sim.interact(smith, InteractAction::RepairAll);
        assert_eq!(
            sim.world
                .get::<Bags>(sim.player_id)
                .unwrap()
                .equipment_wear
                .main_hand,
            Some(0)
        );
    }

    #[test]
    fn train_class_at_alden_refreshes_kit() {
        let mut sim = Sim::new_eastbrook("C", PlayerClass::Warrior);
        let alden = find_template(&sim, "captain_alden").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(alden).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        if let Some(h) = sim.world.get_mut::<Health>(sim.player_id) {
            h.level = 3;
        }
        if let Some(kit) = sim.world.get_mut::<ClassKit>(sim.player_id) {
            kit.known_abilities.clear();
        }

        sim.interact(alden, InteractAction::Talk);
        sim.interact(alden, InteractAction::TrainClass);

        let expected = woc_content::known_abilities_at_level(PlayerClass::Warrior, 3)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            sim.world
                .get::<ClassKit>(sim.player_id)
                .unwrap()
                .known_abilities,
            expected
        );
        assert!(sim.events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message.starts_with("You are trained through level")
        )));
    }

    #[test]
    fn bind_and_hearthstone_from_wolf_run() {
        let mut sim = Sim::new_eastbrook("H", PlayerClass::Warrior);
        let mara = find_template(&sim, "innkeeper_mara").unwrap();
        let (mx, mz) = {
            let t = sim.world.get::<Transform>(mara).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, mx, mz);
        sim.interact(mara, InteractAction::Talk);
        sim.interact(mara, InteractAction::BindHearth);
        let hearth = sim
            .world
            .get::<crate::ecs::components::Hearth>(sim.player_id)
            .unwrap();
        assert!((hearth.x - mx).abs() < 0.01);

        place_player_at(&mut sim, -15.0, 55.0);
        sim.tick = 20;
        let player_id = sim.player_id;
        WorldHost::interact(&mut sim, player_id, 0, InteractAction::UseHearthstone);
        let t = sim.world.get::<Transform>(sim.player_id).unwrap();
        assert!((t.x - mx).abs() < 0.5);
        assert!((t.z - mz).abs() < 0.5);
        assert_eq!(
            sim.world
                .get::<crate::ecs::components::Hearth>(sim.player_id)
                .unwrap()
                .ready_tick,
            20 + 18_000
        );

        place_player_at(&mut sim, -15.0, 55.0);
        let player_id = sim.player_id;
        WorldHost::interact(&mut sim, player_id, 0, InteractAction::UseHearthstone);
        let t = sim.world.get::<Transform>(sim.player_id).unwrap();
        assert!(
            (t.x + 15.0).abs() < 0.5,
            "cooldown must block the second hearth"
        );
    }

    #[test]
    fn vendor_buy_spend_copper() {
        let mut sim = Sim::new_eastbrook("V", PlayerClass::Warrior);
        if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
            p.copper = 100;
        }
        let vendor = find_template(&sim, "trader_wilkes").unwrap();
        let (vx, vz) = {
            let t = sim.world.get::<Transform>(vendor).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, vx, vz);
        sim.interact(vendor, InteractAction::Talk);
        sim.interact(
            vendor,
            InteractAction::Buy {
                item_id: "travelers_ration".into(),
                count: 1,
            },
        );
        assert!(sim.copper() < 100);
        assert!(sim
            .snapshot_for_player(sim.player_id)
            .inventory
            .iter()
            .any(|s| s.item_id == "travelers_ration"));
    }

    #[test]
    fn refuse_to_sell_quest_item() {
        let mut sim = Sim::new_eastbrook("Q", PlayerClass::Warrior);
        let vendor = find_template(&sim, "trader_wilkes").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(vendor).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
            assert!(crate::inventory::grant_into(
                &mut bags.inventory,
                "boar_tusk",
                1
            ));
        }
        let copper_before = sim.copper();
        sim.interact(vendor, InteractAction::Talk);
        let slot = sim
            .world
            .get::<Bags>(sim.player_id)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "boar_tusk"))
            .unwrap() as u8;
        sim.interact(
            vendor,
            InteractAction::Sell {
                bag_slot: slot,
                count: 1,
            },
        );
        assert_eq!(sim.copper(), copper_before);
        assert_eq!(
            crate::inventory::count_item(
                &sim.world.get::<Bags>(sim.player_id).unwrap().inventory,
                "boar_tusk"
            ),
            1
        );
    }

    #[test]
    fn sell_junk_then_buyback() {
        let mut sim = Sim::new_eastbrook("B", PlayerClass::Warrior);
        let vendor = find_template(&sim, "trader_wilkes").unwrap();
        let (x, z) = {
            let t = sim.world.get::<Transform>(vendor).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, x, z);
        if let Some(bags) = sim.world.get_mut::<Bags>(sim.player_id) {
            assert!(crate::inventory::grant_into(
                &mut bags.inventory,
                "wolf_fang",
                2
            ));
        }
        sim.interact(vendor, InteractAction::Talk);
        let slot = sim
            .world
            .get::<Bags>(sim.player_id)
            .unwrap()
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|st| st.item_id == "wolf_fang"))
            .unwrap() as u8;
        let copper_before = sim.copper();
        sim.interact(
            vendor,
            InteractAction::Sell {
                bag_slot: slot,
                count: 2,
            },
        );
        let sold_for = woc_content::item("wolf_fang").unwrap().vendor_sell * 2;
        assert_eq!(sim.copper(), copper_before + sold_for);
        let session = sim.snapshot_for_player(sim.player_id).open_npc.unwrap();
        assert_eq!(session.buyback[0].item_id, "wolf_fang");
        assert_eq!(session.buyback[0].price, sold_for);
        sim.interact(vendor, InteractAction::Buyback { slot: 0 });
        assert_eq!(sim.copper(), copper_before);
        assert_eq!(
            crate::inventory::count_item(
                &sim.world.get::<Bags>(sim.player_id).unwrap().inventory,
                "wolf_fang"
            ),
            2
        );
    }

    #[test]
    fn combat_slice_spawns_wolves() {
        let sim = Sim::new_combat_slice("Test");
        assert!(kind_count(&sim, EntityKind::Mob) >= 4);
        assert!(sim.world.get::<Health>(sim.player_id).unwrap().alive);
    }

    #[test]
    fn kill_wolf_grants_xp_and_loot() {
        let mut sim = Sim::new_combat_slice("Hero");
        let wolf_id = sim
            .world
            .live_ids()
            .find(|&id| sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob))
            .unwrap();
        let (wx, wz) = {
            let t = sim.world.get::<Transform>(wolf_id).unwrap();
            (t.x, t.z)
        };
        place_player_at(&mut sim, wx, wz);
        if let Some(kit) = sim.world.get_mut::<ClassKit>(sim.player_id) {
            kit.resource = 100.0;
        }
        if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
            c.target = Some(wolf_id);
            c.auto_attack = true;
        }
        let intent = PlayerIntent {
            attack: true,
            ability: Some(AbilitySlot::Primary),
            target_id: Some(wolf_id),
            ..Default::default()
        };
        let mut saw_kill = false;
        let mut saw_loot = false;
        for _ in 0..400 {
            let (_snap, events) = sim.tick(intent);
            for e in &events {
                if matches!(e, SimEvent::Kill { .. }) {
                    saw_kill = true;
                }
                if matches!(e, SimEvent::Loot { .. }) {
                    saw_loot = true;
                }
            }
            let level = sim
                .world
                .get::<Health>(sim.player_id)
                .map(|h| h.level)
                .unwrap_or(1);
            if saw_kill && (sim.player_xp() > 0 || level > 1) {
                break;
            }
        }
        assert!(saw_kill);
        let level = sim
            .world
            .get::<Health>(sim.player_id)
            .map(|h| h.level)
            .unwrap_or(1);
        assert!(sim.player_xp() > 0 || level > 1);
        assert!(saw_loot || sim.copper() > 0 || !sim.snapshot().inventory.is_empty());
    }

    #[test]
    fn clear_target_intent_stops_auto_attack() {
        let mut sim = Sim::new_eastbrook("Clearer", PlayerClass::Warrior);
        let wolf_id = sim
            .world
            .live_ids()
            .find(|&id| {
                sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                    && sim
                        .world
                        .get::<Health>(id)
                        .map(|h| h.alive)
                        .unwrap_or(false)
            })
            .unwrap();
        if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
            c.target = Some(wolf_id);
            c.auto_attack = true;
        }
        let _ = sim.tick(PlayerIntent {
            clear_target: true,
            ..Default::default()
        });
        let c = sim.world.get::<Combat>(sim.player_id).unwrap();
        assert!(c.target.is_none());
        assert!(!c.auto_attack);
    }

    #[test]
    fn party_invite_accept_leave_and_chat_roundtrip() {
        let mut sim = Sim::new_empty_eastbrook();
        let a = sim.spawn_player("Alice", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
        let outs = sim.party_invite(a, "Bob");
        assert!(
            outs.iter()
                .any(|m| matches!(m, WsServerMsg::Chat { channel, .. } if channel == "system")),
            "invite notice: {outs:?}"
        );
        let outs = sim.party_accept(b);
        assert!(
            outs.iter().any(|m| matches!(
                m,
                WsServerMsg::PartyUpdate { members } if members.len() == 2
            )),
            "party update: {outs:?}"
        );
        assert_eq!(sim.party_members(a), Some(vec![a, b]));
        let snap = sim.snapshot_for_player(a);
        assert_eq!(snap.party_id, sim.parties.party_id(a));
        assert!(snap.party_id.is_some());

        let outs = sim.chat(a, "party", "pulling");
        assert!(matches!(
            outs.as_slice(),
            [WsServerMsg::Chat {
                channel,
                from,
                text
            }] if channel == "party" && from == "Alice" && text == "pulling"
        ));

        let outs = sim.party_leave(b);
        assert!(
            outs.iter().any(|m| matches!(
                m,
                WsServerMsg::PartyUpdate { members } if members.is_empty()
            )),
            "dissolve: {outs:?}"
        );
        assert!(sim.party_members(a).is_none());
        assert!(sim.snapshot_for_player(a).party_id.is_none());
    }

    #[test]
    fn snapshot_ability_bar_lists_class_kit() {
        let sim = Sim::new_eastbrook("Kit", PlayerClass::Warrior);
        let snap = sim.snapshot();
        assert!(
            snap.ability_bar.len() >= 4,
            "warrior kit should expose ≥4 slots"
        );
        assert_eq!(snap.ability_bar[0].slot, 1);
        assert_eq!(snap.ability_bar[0].ability_id, "heroic_strike");
        assert!(snap.ability_bar[0].known);
        assert_eq!(snap.ability_bar[1].ability_id, "cleave");
        assert!(!snap.ability_bar[1].known, "cleave gated above level 1");
        assert_eq!(snap.protocol_rev, PROTOCOL_REV);
        assert_eq!(snap.combo_points, 0);
        assert!(!snap.stealthed);
        assert_eq!(snap.stance_id, "battle");
        assert_eq!(snap.absorb, 0.0);
    }

    #[test]
    fn toggle_stealth_rogue_only() {
        let mut rogue = Sim::new_eastbrook("Sneak", PlayerClass::Rogue);
        let pid = rogue.player_id;
        woc_protocol::WorldHost::interact(&mut rogue, pid, pid, InteractAction::ToggleStealth);
        assert!(rogue.world.get::<ClassKit>(pid).unwrap().stealthed);
        let snap = rogue.snapshot_for_player(pid);
        assert!(snap.stealthed);
        woc_protocol::WorldHost::interact(&mut rogue, pid, pid, InteractAction::ToggleStealth);
        assert!(!rogue.world.get::<ClassKit>(pid).unwrap().stealthed);

        let mut warrior = Sim::new_eastbrook("Tank", PlayerClass::Warrior);
        let wid = warrior.player_id;
        woc_protocol::WorldHost::interact(&mut warrior, wid, wid, InteractAction::ToggleStealth);
        assert!(!warrior.world.get::<ClassKit>(wid).unwrap().stealthed);
    }

    #[test]
    fn cycle_stance_warrior_only() {
        let mut warrior = Sim::new_eastbrook("Tank", PlayerClass::Warrior);
        let wid = warrior.player_id;
        assert_eq!(
            warrior
                .world
                .get::<ClassKit>(wid)
                .unwrap()
                .stance_id
                .as_deref(),
            Some("battle")
        );
        woc_protocol::WorldHost::interact(&mut warrior, wid, wid, InteractAction::CycleStance);
        assert_eq!(
            warrior
                .world
                .get::<ClassKit>(wid)
                .unwrap()
                .stance_id
                .as_deref(),
            Some("defensive")
        );
        let snap = warrior.snapshot_for_player(wid);
        assert_eq!(snap.stance_id, "defensive");
        woc_protocol::WorldHost::interact(&mut warrior, wid, wid, InteractAction::CycleStance);
        assert_eq!(
            warrior
                .world
                .get::<ClassKit>(wid)
                .unwrap()
                .stance_id
                .as_deref(),
            Some("battle")
        );

        let mut mage = Sim::new_eastbrook("Glass", PlayerClass::Mage);
        let mid = mage.player_id;
        woc_protocol::WorldHost::interact(&mut mage, mid, mid, InteractAction::CycleStance);
        assert!(mage.world.get::<ClassKit>(mid).unwrap().stance_id.is_none());
    }

    #[test]
    fn toggle_form_druid_and_shaman() {
        let mut druid = Sim::new_eastbrook("Cat", PlayerClass::Druid);
        let did = druid.player_id;
        woc_protocol::WorldHost::interact(&mut druid, did, did, InteractAction::ToggleForm);
        assert_eq!(
            druid
                .world
                .get::<ClassKit>(did)
                .unwrap()
                .stance_id
                .as_deref(),
            Some("travel_form")
        );
        assert!((crate::combat::move_speed_mult(&druid.world, did) - 1.4).abs() < 1e-3);
        woc_protocol::WorldHost::interact(&mut druid, did, did, InteractAction::ToggleForm);
        assert!(druid
            .world
            .get::<ClassKit>(did)
            .unwrap()
            .stance_id
            .is_none());

        let mut shaman = Sim::new_eastbrook("Wolf", PlayerClass::Shaman);
        let sid = shaman.player_id;
        woc_protocol::WorldHost::interact(&mut shaman, sid, sid, InteractAction::ToggleForm);
        assert_eq!(
            shaman
                .world
                .get::<ClassKit>(sid)
                .unwrap()
                .stance_id
                .as_deref(),
            Some("ghost_wolf")
        );
    }

    #[test]
    fn death_release_spirit_respawns_at_eastbrook_graveyard() {
        let gy = woc_content::graveyard("eastbrook_graveyard").expect("eastbrook graveyard");
        let mut sim = Sim::new_eastbrook("Deadman", PlayerClass::Warrior);
        let pid = sim.player_id;
        let death_x = 22.0;
        let death_z = -20.0;
        place_player_at(&mut sim, death_x, death_z);
        if let Some(h) = sim.world.get_mut::<Health>(pid) {
            h.hp = 0.0;
        }
        if let Some(c) = sim.world.get_mut::<Combat>(pid) {
            c.auto_attack = true;
            c.target = Some(99);
        }

        let (_snap, events) = sim.tick(PlayerIntent::default());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SimEvent::PlayerDied { player } if *player == pid)),
            "expected PlayerDied event"
        );
        let snap = sim.snapshot_for_player(pid);
        assert!(snap.is_dead, "snapshot must reflect is_dead");
        assert!(!sim.world.get::<Health>(pid).unwrap().alive);
        assert!(!sim.world.get::<Combat>(pid).unwrap().auto_attack);

        let wolf_id = sim.world.live_ids().find(|&id| {
            sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                && sim
                    .world
                    .get::<Health>(id)
                    .map(|h| h.alive)
                    .unwrap_or(false)
        });
        if let Some(wid) = wolf_id {
            let hp_before = sim.world.get::<Health>(wid).unwrap().hp;
            let intent = PlayerIntent {
                attack: true,
                ability: Some(AbilitySlot::Primary),
                target_id: Some(wid),
                ..Default::default()
            };
            let (_s, ev) = sim.tick(intent);
            let hp_after = sim.world.get::<Health>(wid).unwrap().hp;
            assert_eq!(hp_before, hp_after, "dead player must not deal damage");
            assert!(
                !ev.iter().any(|e| matches!(
                    e,
                    SimEvent::Damage { source, .. } if *source == pid
                )),
                "no Damage from dead player"
            );
        }

        assert!(sim.release_spirit(pid), "release_spirit should succeed");
        let t = sim.world.get::<Transform>(pid).unwrap();
        let h = sim.world.get::<Health>(pid).unwrap();
        assert!(h.alive);
        assert!((t.x - gy.x).abs() < 1e-5, "x {} vs gy {}", t.x, gy.x);
        assert!((t.z - gy.z).abs() < 1e-5, "z {} vs gy {}", t.z, gy.z);
        assert!((h.hp - h.hp_max).abs() < 1e-5);
        let snap = sim.snapshot_for_player(pid);
        assert!(!snap.is_dead);
    }

    #[test]
    fn same_seed_same_snapshots() {
        let mut a = Sim::new_combat_slice("A");
        let mut b = Sim::new_combat_slice("A");
        let intent = PlayerIntent {
            move_x: 0.3,
            move_z: 1.0,
            facing: 0.4,
            attack: false,
            ability: None,
            target_id: None,
            ..Default::default()
        };
        for _ in 0..60 {
            let (sa, _) = a.tick(intent);
            let (sb, _) = b.tick(intent);
            assert_eq!(sa.tick, sb.tick);
            assert_eq!(sa.entities.len(), sb.entities.len());
            for (ea, eb) in sa.entities.iter().zip(sb.entities.iter()) {
                assert_eq!(ea.id, eb.id);
                assert!((ea.x - eb.x).abs() < 1e-5);
                assert!((ea.z - eb.z).abs() < 1e-5);
                assert!((ea.hp - eb.hp).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn hunter_summon_shows_pet_in_snapshot_and_dismisses() {
        let mut sim = Sim::new_eastbrook("Hunt", PlayerClass::Hunter);
        let pid = sim.player_id;
        sim.interact(pid, InteractAction::SummonPet);
        let snap = sim.snapshot();
        let pet = snap
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Pet)
            .expect("snapshot should include pet entity");
        assert_eq!(pet.template_id.as_deref(), Some("hunter_wolf"));
        assert!(pet.alive);
        sim.interact(pid, InteractAction::DismissPet);
        let snap = sim.snapshot();
        assert!(!snap.entities.iter().any(|e| e.kind == EntityKind::Pet));
    }

    #[test]
    fn pet_tick_damages_player_target_via_sim() {
        let mut sim = Sim::new_eastbrook("Hunt", PlayerClass::Hunter);
        let pid = sim.player_id;
        let mob_id = sim
            .world
            .live_ids()
            .find(|&id| {
                sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Mob)
                    && sim
                        .world
                        .get::<Health>(id)
                        .map(|h| h.alive)
                        .unwrap_or(false)
            })
            .expect("mob");
        let (px, pz) = {
            let t = sim.world.get::<Transform>(pid).unwrap();
            (t.x, t.z)
        };
        if let Some(t) = sim.world.get_mut::<Transform>(mob_id) {
            t.x = px + 1.5;
            t.z = pz;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        sim.interact(pid, InteractAction::SummonPet);
        let pet_id = sim
            .world
            .live_ids()
            .find(|&id| sim.world.get::<Identity>(id).map(|i| i.kind) == Some(EntityKind::Pet))
            .expect("pet");
        if let Some(t) = sim.world.get_mut::<Transform>(pet_id) {
            t.x = px + 1.5;
            t.z = pz;
            t.y = crate::ecs::spawn::ground_at(t.x, t.z);
        }
        if let Some(c) = sim.world.get_mut::<Combat>(pet_id) {
            c.swing_timer = 0.0;
        }
        let hp_before = sim.world.get::<Health>(mob_id).unwrap().hp;
        let intent = PlayerIntent {
            attack: true,
            target_id: Some(mob_id),
            ..Default::default()
        };
        for _ in 0..40 {
            let _ = sim.tick(intent);
        }
        let hp_after = sim.world.get::<Health>(mob_id).unwrap().hp;
        assert!(
            hp_after < hp_before,
            "expected pet+player damage ({hp_after} < {hp_before})"
        );
    }

    #[test]
    fn park_then_resume_keeps_entity_id_and_position() {
        let mut sim = Sim::new_empty_eastbrook();
        let id = sim.spawn_player("Ada", PlayerClass::Warrior).unwrap();
        if let Some(d) = sim.world.get_mut::<crate::ecs::components::Durable>(id) {
            d.durable_id = Some("char-ada".into());
        }
        if let Some(t) = sim.world.get_mut::<Transform>(id) {
            t.x = 12.0;
            t.z = 8.0;
        }
        if let Some(h) = sim.world.get_mut::<Health>(id) {
            h.hp = 17.0;
        }
        sim.park_player(id);
        assert!(sim.world.contains(id), "park must keep the entity");
        assert!(
            !sim.intents.contains_key(&id),
            "park must drop the intent slot"
        );
        assert_eq!(sim.world.get::<Health>(id).unwrap().hp, 17.0);
        let resumed = sim.resume_player("char-ada").expect("resume parked");
        assert_eq!(resumed, id);
        let t = sim.world.get::<Transform>(id).unwrap();
        assert_eq!(t.x, 12.0);
        assert_eq!(t.z, 8.0);
        assert!(sim.intents.contains_key(&id));
        assert!(sim.resume_player("char-ada").is_none(), "already connected");
        sim.park_player(id);
        let state = sim.export_player_state(id).unwrap();
        let via_spawn = sim
            .spawn_player_with_state("Ada", PlayerClass::Warrior, &state)
            .expect("spawn resumes parked");
        assert_eq!(via_spawn, id);
        assert_eq!(sim.world.get::<Health>(id).unwrap().hp, 17.0);
    }

    #[test]
    fn snapshot_includes_nearby_mob_and_omits_far_mob() {
        let mut sim = Sim::new_eastbrook("Scout", PlayerClass::Warrior);
        let pid = sim.player_id;
        let (px, pz) = {
            let t = sim.world.get::<Transform>(pid).unwrap();
            (t.x, t.z)
        };
        let near = sim.world.next_id();
        crate::ecs::spawn::create_mob_from_template(
            &mut sim.world,
            near,
            "young_wolf",
            px + 5.0,
            pz,
        )
        .unwrap();
        let far = sim.world.next_id();
        crate::ecs::spawn::create_mob_from_template(
            &mut sim.world,
            far,
            "young_wolf",
            px,
            pz + 200.0,
        )
        .unwrap();
        let snap = sim.snapshot_for_player(pid);
        assert!(
            snap.entities.iter().any(|e| e.id == near),
            "mob at 5 yd must be in the snapshot"
        );
        assert!(
            !snap.entities.iter().any(|e| e.id == far),
            "mob at 200 yd same zone must be omitted"
        );
        assert!(
            crate::ecs::components::dist2d(&sim.world, pid, far).unwrap() > SNAPSHOT_AOI_RADIUS
        );
    }

    #[test]
    fn snapshot_always_includes_zone_npcs() {
        let mut sim = Sim::new_eastbrook("Talker", PlayerClass::Warrior);
        let pid = sim.player_id;
        let zone = sim.world.get::<Identity>(pid).unwrap().zone_id.clone();
        let far_npc = sim
            .world
            .live_ids()
            .find(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .is_some_and(|i| i.kind == EntityKind::Npc && i.zone_id == zone)
            })
            .expect("zone npc");
        let (px, pz) = {
            let p = sim.world.get::<Transform>(pid).unwrap();
            (p.x, p.z)
        };
        if let Some(t) = sim.world.get_mut::<Transform>(far_npc) {
            t.x = px;
            t.z = pz + 200.0;
        }
        let snap = sim.snapshot_for_player(pid);
        assert!(
            snap.entities.iter().any(|e| e.id == far_npc),
            "zone NPCs stay in the snapshot so talk/quest still works"
        );
    }

    #[test]
    fn instance_independent_loot_tags_all_piles() {
        let mut sim = Sim::new_eastbrook("InstLoot", PlayerClass::Warrior);
        let instance_id = "mirefen_barrow#loot-test".to_string();
        if let Some(i) = sim.world.get_mut::<InstanceAt>(sim.player_id) {
            i.instance_id = Some(instance_id.clone());
        }
        let mob_id = sim.world.next_id();
        spawn::create_mob_from_template(&mut sim.world, mob_id, "barrow_hag", 0.0, 0.0)
            .expect("barrow_hag");
        if let Some(i) = sim.world.get_mut::<InstanceAt>(mob_id) {
            i.instance_id = Some(instance_id.clone());
        }
        if let Some(h) = sim.world.get_mut::<Health>(mob_id) {
            h.hp = 1.0;
            h.hp_max = 1.0;
        }
        place_player_at(&mut sim, 2.5, 0.0);
        if let Some(kit) = sim.world.get_mut::<ClassKit>(sim.player_id) {
            kit.resource = 100.0;
        }
        if let Some(c) = sim.world.get_mut::<Combat>(sim.player_id) {
            c.target = Some(mob_id);
            c.auto_attack = true;
        }
        let intent = PlayerIntent {
            attack: true,
            ability: Some(AbilitySlot::Primary),
            target_id: Some(mob_id),
            ..Default::default()
        };
        let mut saw_kill = false;
        for _ in 0..400 {
            let (_snap, events) = sim.tick(intent);
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::Kill { victim, .. } if *victim == mob_id))
            {
                saw_kill = true;
                break;
            }
        }
        assert!(saw_kill, "barrow_hag should die in combat");
        let piles: Vec<(Option<String>, Option<String>)> = sim
            .world
            .ids::<LootPile>()
            .into_iter()
            .filter_map(|id| {
                let item = sim.world.get::<LootPile>(id)?.item.clone();
                let inst = sim.world.get::<InstanceAt>(id)?.instance_id.clone();
                Some((item, inst))
            })
            .filter(|(item, _)| matches!(item.as_deref(), Some("hag_claw") | Some("hag_focus")))
            .collect();
        assert_eq!(piles.len(), 2, "expected two independent loot piles");
        assert!(
            piles
                .iter()
                .all(|(_, inst)| inst.as_ref() == Some(&instance_id)),
            "every pile must inherit the killer's instance"
        );
        assert!(piles.iter().any(|(i, _)| i.as_deref() == Some("hag_claw")));
        assert!(piles.iter().any(|(i, _)| i.as_deref() == Some("hag_focus")));
    }
}
