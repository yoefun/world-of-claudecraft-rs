//! Sim coordinator: tick loop, intents, snapshots.
//!
//! # Tick phases (locked order — do not reorder)
//!
//! See [`crate::context::TICK_PHASES`]. Actual `tick` execution:
//! 1. `apply_intents_motion` — per-player intent + motion
//! 2. `player_combat` — per-player combat
//! 3. `mob_ai_combat` — mob AI (nearest living player) + mob combat
//! 4. `kill_rewards` — XP/quest credit to killer + loot spawn
//! 5. `loot_pickup` — proximity pickup for all players
//! 6. `build_snapshot` — snapshot for primary/`snapshot_for` viewer

use std::collections::HashMap;

use crate::combat::{
    collect_pending_mob_kills, grant_xp, spawn_mob_loot, tick_auras, try_pickup_loot,
    update_mob_combat, update_player_combat,
};
use crate::context::SimContext;
use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Health, Identity, InstanceAt, LootTable, Motion, Owner,
    Progress, QuestLog, Transform,
};
use crate::ecs::World;
use crate::entity::{create_player, Entity, QuestState};
use crate::interaction::vendor_snapshot;
use crate::mob::{tick_mob_respawns, update_mob_ai};
use crate::pet::{dismiss_pet, tick_pets};
use crate::player_motion::step_player_motion;
use crate::quests::on_mob_killed;
use crate::rng::Rng;
use crate::social::chat::{handle_chat, ChatEffect};
use crate::social::party::{kill_credit_share, PartyEffect, PartyRoster};
use crate::types::xp_to_next;
use crate::world::WORLD_SEED;
use crate::zones::populate_all_overworld;
use woc_content::{ability, class_def, PlayerClass, EASTBROOK};
use woc_protocol::{
    AbilityBarSlot, AuraSnapshot, CastSnapshot, EntityId, EntityKind, EntitySnapshot,
    EquipmentSnapshot, InteractAction, InvSlotSnapshot, PlayerIntent, PlayerProgress,
    QuestLogEntry, SimEvent, TickSnapshot, WsServerMsg, DT, PROTOCOL_REV,
};

/// Max concurrent player entities on one Eastbrook realm (dev scaffold).
pub const MAX_REALM_PLAYERS: usize = 8;

pub struct Sim {
    pub tick: u64,
    pub seed: u32,
    pub rng: Rng,
    pub entities: Vec<Entity>,
    /// Sparse-column mirror of `entities` (dual-write during ECS migration).
    pub world: World,
    /// Parallel index: `EntityId` → slot in `entities`. Kept in sync on spawn/despawn.
    pub by_id: HashMap<EntityId, usize>,
    pub next_id: EntityId,
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
    pub(crate) fn rebuild_world(&mut self) {
        let mut world = World::new();
        world.set_next_id(self.next_id);
        for entity in &self.entities {
            crate::ecs::spawn::sync_entity_to_world(&mut world, entity);
        }
        self.world = world;
    }

    pub(crate) fn reindex(&mut self) {
        self.by_id.clear();
        for (i, e) in self.entities.iter().enumerate() {
            self.by_id.insert(e.id, i);
        }
    }

    pub(crate) fn push_entity(&mut self, e: Entity) {
        self.by_id.insert(e.id, self.entities.len());
        crate::ecs::spawn::sync_entity_to_world(&mut self.world, &e);
        self.entities.push(e);
        self.world.set_next_id(self.next_id);
    }

    pub fn entity_index(&self, id: EntityId) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    fn entity_ref(&self, id: EntityId) -> Option<&Entity> {
        let i = self.entity_index(id)?;
        self.entities.get(i).filter(|e| e.id == id)
    }

    fn entity_mut_ref(&mut self, id: EntityId) -> Option<&mut Entity> {
        let i = self.entity_index(id)?;
        self.entities.get_mut(i).filter(|e| e.id == id)
    }
    /// Continuous overworld (all zone bands) with no player. Online sticky realm.
    pub fn new_empty_eastbrook() -> Self {
        let seed = WORLD_SEED;
        let mut rng = Rng::new(seed);
        let mut next_id = 1u32;
        let mut entities = Vec::new();
        populate_all_overworld(&mut entities, &mut next_id, &mut rng);

        let mut sim = Self {
            tick: 0,
            seed,
            rng,
            entities,
            world: World::new(),
            by_id: HashMap::new(),
            next_id,
            player_id: 0,
            events: Vec::new(),
            intents: HashMap::new(),
            parties: PartyRoster::new(),
            mail: crate::mail::Mailbox::new(),
            market: crate::market::AuctionHouse::new(),
            loot_rules: crate::social::LootRules::default(),
            pvp: crate::pvp::PvpState::default(),
        };
        sim.reindex();
        sim.rebuild_world();
        sim
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
        let players = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Player)
            .count();
        if players >= MAX_REALM_PLAYERS {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let offset = players as f32 * 1.5;
        let mut player = create_player(
            id,
            name,
            class,
            EASTBROOK.player_spawn_x + offset,
            EASTBROOK.player_spawn_z,
        );
        player.y = Entity::ground_at(player.x, player.z);
        self.push_entity(player);
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
        let players = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Player)
            .count();
        if players >= MAX_REALM_PLAYERS {
            return None;
        }
        // Reject duplicate durable character already in realm.
        if let Some(ref did) = state.durable_id {
            if self.entities.iter().any(|e| {
                e.kind == EntityKind::Player && e.durable_id.as_deref() == Some(did.as_str())
            }) {
                return None;
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut player = crate::persist_state::create_player_from_state(id, name, class, state);
        // Virgin / eastbrook spawn: slight offset so multiplayer doesn't stack.
        if state.is_virgin()
            || (player.zone_id == "eastbrook"
                && state.pos_x.abs() < 0.01
                && state.pos_z.abs() < 0.01)
        {
            let offset = players as f32 * 1.5;
            player.x = EASTBROOK.player_spawn_x + offset;
            player.z = EASTBROOK.player_spawn_z;
            player.y = Entity::ground_at(player.x, player.z);
            player.home_x = player.x;
            player.home_z = player.z;
        } else {
            player.y = Entity::ground_at(player.x, player.z);
        }
        self.push_entity(player);
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
        let world = crate::ecs::spawn::world_from_entities(&self.entities);
        crate::persist_state::export_player_state(&world, player_id)
    }

    /// Remove a player from the realm (disconnect). Does not recreate Eastbrook.
    pub fn despawn_player(&mut self, player_id: EntityId) {
        let _ = dismiss_pet(
            &mut self.world,
            &mut self.entities,
            player_id,
            &mut self.events,
        );
        let _ = self.parties.on_despawn(player_id);
        self.entities
            .retain(|e| !(e.id == player_id && e.kind == EntityKind::Player));
        self.world.despawn(player_id);
        self.reindex();
        self.rebuild_world();
        self.intents.remove(&player_id);
        if self.player_id == player_id {
            self.player_id = self
                .entities
                .iter()
                .find(|e| e.kind == EntityKind::Player)
                .map(|e| e.id)
                .unwrap_or(0);
        }
    }

    /// Party invite by target player name.
    pub fn party_invite(&mut self, player_id: EntityId, name: &str) -> Vec<WsServerMsg> {
        self.rebuild_world();
        let effects = self.parties.invite(player_id, name, &self.world);
        map_party_effects(effects)
    }

    /// Accept a pending party invite.
    pub fn party_accept(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        self.rebuild_world();
        let effects = self.parties.accept(player_id, &self.world);
        map_party_effects(effects)
    }

    /// Leave the current party (dissolves when size drops below 2).
    pub fn party_leave(&mut self, player_id: EntityId) -> Vec<WsServerMsg> {
        map_party_effects(self.parties.leave(player_id))
    }

    /// Say / party chat.
    pub fn chat(&mut self, player_id: EntityId, channel: &str, text: &str) -> Vec<WsServerMsg> {
        self.rebuild_world();
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
        self.entities
            .iter()
            .filter(|e| e.kind == EntityKind::Player)
            .count()
    }

    pub fn player(&self) -> Option<&Entity> {
        self.entity_ref(self.player_id)
    }

    pub fn player_mut(&mut self) -> Option<&mut Entity> {
        let id = self.player_id;
        self.entity_mut_ref(id)
    }

    /// Convenience: primary player copper (0 if none).
    pub fn copper(&self) -> u32 {
        self.player().map(|p| p.copper).unwrap_or(0)
    }

    /// Convenience: primary player XP (0 if none).
    pub fn player_xp(&self) -> u32 {
        self.player().map(|p| p.xp).unwrap_or(0)
    }

    pub fn interact(&mut self, target_id: EntityId, action: InteractAction) {
        woc_protocol::WorldHost::interact(self, self.player_id, target_id, action);
    }

    /// Tab-cycle living hostile mobs for the primary player.
    pub fn tab_target(&mut self) -> Option<EntityId> {
        self.rebuild_world();
        let id = crate::targeting::tab_target(&self.world, self.player_id)?;
        if let Some(c) = self.world.get_mut::<Combat>(self.player_id) {
            c.target = Some(id);
        }
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        Some(id)
    }

    /// Clear the primary player's current target and stop auto-attack (Esc).
    pub fn clear_target(&mut self) {
        if let Some(p) = self.player_mut() {
            p.target = None;
            p.auto_attack = false;
            p.cast = None;
        }
    }

    pub fn grant_item(
        &mut self,
        player_id: EntityId,
        item_id: &str,
        count: u32,
    ) -> Result<(), &'static str> {
        self.rebuild_world();
        crate::inventory::grant_item(
            &mut self.world,
            player_id,
            item_id,
            count,
            &mut self.events,
        )?;
        crate::quests::on_inventory_changed(&mut self.world, player_id, &mut self.events);
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        Ok(())
    }

    /// Release a dead player's spirit: respawn at the Eastbrook graveyard.
    ///
    /// Call from the interact/toast UI path when the player confirms release.
    /// Returns `false` if the player is missing or still alive.
    pub fn release_spirit(&mut self, player_id: EntityId) -> bool {
        self.rebuild_world();
        let ok = crate::spirit::release_spirit(&mut self.world, player_id, &mut self.events);
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        ok
    }

    /// Build a context bag for leaf modules (emit / lookup / mutate).
    pub fn context(&mut self) -> SimContext<'_> {
        SimContext {
            events: &mut self.events,
            entities: &mut self.entities,
            by_id: &self.by_id,
            rng: &mut self.rng,
            next_id: &mut self.next_id,
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
        self.reindex();

        let player_ids: Vec<EntityId> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Player)
            .map(|e| e.id)
            .collect();

        self.rebuild_world();
        // Phase 1: apply intents + motion (World is source of truth this phase)
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
        }
        // Phase 2: player combat
        for &pid in &player_ids {
            let ability = self.intents.get(&pid).and_then(|i| i.ability);
            update_player_combat(pid, &mut self.world, ability, &mut self.events);
        }
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);

        // Pet AI (after player combat; keeps TICK_PHASES fingerprint stable).
        let dropped = tick_pets(&mut self.world, &mut self.events);
        if !dropped.is_empty() {
            self.entities.retain(|e| !dropped.contains(&e.id));
        }
        self.reindex();
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);

        // Phase 3: mob AI + combat (focus nearest living player)
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

        // Aura/timer decay (hook into tick after combat; keeps TICK_PHASES fingerprint stable).
        tick_auras(&mut self.world, &mut self.events);
        tick_mob_respawns(&mut self.world, DT);
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);

        // Phase 4: kill rewards → killer (+ party share stub)
        let rewards = collect_pending_mob_kills(&self.events, &self.entities);
        for reward in rewards {
            let mut recipients = vec![reward.killer];
            for mate in kill_credit_share(&self.parties, &self.world, reward.killer) {
                if !recipients.contains(&mate) {
                    recipients.push(mate);
                }
            }
            for rid in recipients {
                if let Some(pi) = self.entity_index(rid) {
                    if self.entities[pi].kind == EntityKind::Player {
                        grant_xp(&mut self.entities[pi], reward.xp, &mut self.events);
                        if let Some(ref tid) = reward.template_id {
                            crate::ecs::spawn::sync_entity_to_world(
                                &mut self.world,
                                &self.entities[pi],
                            );
                            on_mob_killed(&mut self.world, rid, tid, &mut self.events);
                            crate::ecs::spawn::apply_world_to_entity(
                                &self.world,
                                &mut self.entities[pi],
                            );
                            crate::worldboss::on_boss_killed_entity(
                                &mut self.entities[pi],
                                tid,
                                &mut self.events,
                            );
                        }
                    }
                }
            }
            spawn_mob_loot(
                &mut self.next_id,
                &mut self.entities,
                &mut self.rng,
                reward.template_id.as_deref(),
                reward.x,
                reward.z,
            );
        }
        self.reindex();
        self.rebuild_world();
        // ws-death: finalize player deaths (corpse + PlayerDied) after kill rewards
        crate::death::on_player_death_check(&mut self.world, &mut self.events);
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);

        // PvP duel resolve + honor (does not add a locked tick phase).
        crate::pvp::tick_pvp(&mut self.pvp, &mut self.world, &mut self.events);
        self.market
            .tick_expire(self.tick, &mut self.world, &mut self.mail);
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        // Sync entity-only pvp/market writes before World-based loot pickup.
        self.rebuild_world();

        // Phase 5: loot pickup for all players
        for &pid in &player_ids {
            try_pickup_loot(pid, &mut self.world, &mut self.entities, &mut self.events);
        }
        crate::ecs::spawn::apply_world_to_entities(&self.world, &mut self.entities);
        self.reindex();
        self.rebuild_world();

        // Phase 6: snapshot
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
        let level = world
            .get::<Health>(player_id)
            .map(|h| h.level)
            .unwrap_or(1);
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
            })
            .unwrap_or_default();

        let quest_log = world
            .get::<QuestLog>(player_id)
            .map(|q| {
                q.quest_log
                    .iter()
                    .map(|quest| QuestLogEntry {
                        quest_id: quest.quest_id.clone(),
                        state: match quest.state {
                            QuestState::Active => "active",
                            QuestState::Ready => "ready",
                            QuestState::Completed => "completed",
                        }
                        .to_string(),
                        counts: quest.counts.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let open_vendor = vendor_snapshot(world, player_id);

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
            .filter(|&id| snapshot_includes_entity(world, viewer_instance.as_deref(), id))
            .filter_map(|id| entity_snapshot(world, id))
            .collect();

        TickSnapshot {
            tick: self.tick,
            player_id,
            entities,
            progress: PlayerProgress {
                xp: world
                    .get::<Progress>(player_id)
                    .map(|p| p.xp)
                    .unwrap_or(0),
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
            zone_id: world
                .get::<Identity>(player_id)
                .map(|i| i.zone_id.clone())
                .unwrap_or_else(|| "eastbrook".into()),
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
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            mail: self.mail.snapshot_for_entity(player_id, world),
            market: self.market.snapshot_public(),
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
        }
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

fn snapshot_includes_entity(
    world: &World,
    viewer_instance: Option<&str>,
    id: EntityId,
) -> bool {
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
            let known = kit
                .known_abilities
                .iter()
                .any(|id| id == entry.ability_id);
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
    use woc_protocol::{AbilitySlot, InteractAction, WorldHost, WsServerMsg};

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
            .filter(|&id| snapshot_includes_entity(&sim.world, viewer_instance.as_deref(), id))
            .filter(|&id| entity_snapshot(&sim.world, id).is_some())
            .collect();
        let snap_ids: Vec<EntityId> = snap.entities.iter().map(|e| e.id).collect();
        assert_eq!(snap_ids, expected);
        assert_eq!(snap.entities.len(), expected.len());
    }

    #[test]
    fn tick_phase_order_fingerprint_locked() {
        assert_eq!(TICK_PHASES.len(), 6);
        assert_eq!(TICK_PHASES[0], "apply_intents_motion");
        assert_eq!(TICK_PHASES[5], "build_snapshot");
        // Locked fingerprint — update deliberately if phases change.
        assert_eq!(tick_phase_fingerprint(), 1724209595281213949u64);
    }

    #[test]
    fn sim_context_emit_and_lookup() {
        let mut sim = Sim::new_eastbrook("Ctx", PlayerClass::Warrior);
        let pid = sim.player_id;
        {
            let mut ctx = sim.context();
            assert!(ctx.entity(pid).is_some());
            ctx.emit(SimEvent::Toast {
                message: "hi".into(),
            });
            assert_eq!(ctx.player_ids().len(), 1);
        }
        assert_eq!(sim.events.len(), 1);
    }

    #[test]
    fn entity_index_matches_scan() {
        let sim = Sim::new_eastbrook("Idx", PlayerClass::Warrior);
        for e in &sim.entities {
            assert_eq!(
                sim.entity_index(e.id).map(|i| sim.entities[i].id),
                Some(e.id)
            );
        }
        assert!(sim.entity_index(u32::MAX).is_none());
    }

    #[test]
    fn loot_has_no_bags_column() {
        let mut sim = Sim::new_eastbrook("LootCol", PlayerClass::Warrior);
        let id = sim.next_id;
        sim.next_id += 1;
        sim.push_entity(crate::entity::create_loot(id, 0.0, 0.0, 5, None));
        assert!(sim
            .world
            .get::<crate::ecs::components::LootPile>(id)
            .is_some());
        assert!(sim.world.get::<crate::ecs::components::Bags>(id).is_none());
        assert!(sim.world.get::<crate::ecs::components::Bank>(id).is_none());
        assert!(sim
            .world
            .get::<crate::ecs::components::ClassKit>(id)
            .is_none());
        let player_id = sim.player_id;
        assert!(sim
            .world
            .get::<crate::ecs::components::Bags>(player_id)
            .is_some());
        assert!(sim
            .world
            .get::<crate::ecs::components::LootPile>(player_id)
            .is_none());
    }

    #[test]
    fn eastbrook_spawns_npcs_and_mobs_from_content() {
        let sim = Sim::new_eastbrook("Tester", PlayerClass::Warrior);
        let npc_count = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Npc)
            .count();
        let mob_count = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob)
            .count();
        assert!(npc_count >= 3, "expected town NPCs, got {npc_count}");
        assert!(mob_count >= 4, "expected camps, got {mob_count}");
    }

    #[test]
    fn sticky_spawn_does_not_reset_npcs() {
        let mut sim = Sim::new_empty_eastbrook();
        let npc_before = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Npc)
            .count();
        let mob_before = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob)
            .count();
        let a = sim.spawn_player("A", PlayerClass::Warrior).unwrap();
        let b = sim.spawn_player("B", PlayerClass::Mage).unwrap();
        assert_ne!(a, b);
        assert_eq!(sim.player_count(), 2);
        let npc_after = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Npc)
            .count();
        let mob_after = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob)
            .count();
        assert_eq!(npc_before, npc_after);
        assert_eq!(mob_before, mob_after);
        sim.despawn_player(a);
        assert_eq!(sim.player_count(), 1);
        assert_eq!(
            sim.entities
                .iter()
                .filter(|e| e.kind == EntityKind::Npc)
                .count(),
            npc_before
        );
    }

    #[test]
    fn all_nine_classes_spawn() {
        for class in PlayerClass::ALL {
            let sim = Sim::new_eastbrook("C", class);
            let p = sim.player().unwrap();
            assert!(p.alive);
            assert!(p.hp_max > 0.0);
            assert!(p.equipment.main_hand.is_some());
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
        if let Some(p) = sim.entity_mut_ref(winner) {
            p.x = 0.0;
            p.z = 0.0;
            p.y = Entity::ground_at(p.x, p.z);
        }
        if let Some(p) = sim.entity_mut_ref(loser) {
            p.x = 1.0;
            p.z = 0.0;
            p.y = Entity::ground_at(p.x, p.z);
            p.hp = 1.0;
        }
        sim.rebuild_world();

        let mut duel_events = Vec::new();
        crate::pvp::challenge_duel(&mut sim.pvp, &sim.world, winner, loser).unwrap();
        crate::pvp::accept_duel(
            &mut sim.pvp,
            &sim.world,
            loser,
            winner,
            &mut duel_events,
        )
        .unwrap();

        let (wx, wz) = {
            let w = sim.entity_ref(winner).unwrap();
            (w.x, w.z)
        };
        let loot_id = sim.next_id;
        sim.next_id += 1;
        sim.push_entity(crate::entity::create_loot(loot_id, wx, wz, 7, None));

        assert_eq!(sim.entity_ref(winner).unwrap().honor, 0);

        let (_snap, events) = sim.tick(PlayerIntent::default());

        assert_eq!(
            sim.entity_ref(winner).unwrap().honor,
            crate::pvp::HONOR_PER_KILL,
            "entity-only pvp honor must survive Phase 5 apply_world_to_entities"
        );
        assert_eq!(sim.copper(), 7, "loot copper should apply in the same tick");
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
        let giver = sim
            .entities
            .iter()
            .find(|e| e.template_id.as_deref() == Some("captain_alden"))
            .unwrap()
            .id;
        sim.interact(
            giver,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );

        let wolf_ids: Vec<EntityId> = sim
            .entities
            .iter()
            .filter(|e| e.template_id.as_deref() == Some("young_wolf") && e.alive)
            .map(|e| e.id)
            .take(3)
            .collect();
        assert_eq!(wolf_ids.len(), 3);

        for wid in wolf_ids {
            let (wx, wz) = {
                let w = sim.entities.iter().find(|e| e.id == wid).unwrap();
                (w.x, w.z)
            };
            if let Some(p) = sim.player_mut() {
                p.x = wx;
                p.z = wz;
                p.y = Entity::ground_at(p.x, p.z);
                p.resource = 100.0;
                p.target = Some(wid);
                p.auto_attack = true;
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
            .player()
            .unwrap()
            .quest_log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == QuestState::Ready);
        assert!(ready, "quest should be ready after 3 kills");

        let (gx, gz) = {
            let g = sim.entities.iter().find(|e| e.id == giver).unwrap();
            (g.x, g.z)
        };
        if let Some(p) = sim.player_mut() {
            p.x = gx;
            p.z = gz;
            p.y = Entity::ground_at(p.x, p.z);
        }

        sim.interact(
            giver,
            InteractAction::TurnInQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );
        let log = WorldHost::snapshot_for(&sim, sim.player_id).quest_log;
        assert!(log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == "completed"));
    }

    #[test]
    fn vendor_buy_spend_copper() {
        let mut sim = Sim::new_eastbrook("V", PlayerClass::Warrior);
        sim.player_mut().unwrap().copper = 100;
        let vendor = sim
            .entities
            .iter()
            .find(|e| e.template_id.as_deref() == Some("trader_wilkes"))
            .unwrap()
            .id;
        let (vx, vz) = {
            let v = sim.entities.iter().find(|e| e.id == vendor).unwrap();
            (v.x, v.z)
        };
        if let Some(p) = sim.player_mut() {
            p.x = vx;
            p.z = vz;
            p.y = Entity::ground_at(p.x, p.z);
        }
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
    fn combat_slice_spawns_wolves() {
        let sim = Sim::new_combat_slice("Test");
        let wolves = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob)
            .count();
        assert!(wolves >= 4);
        assert!(sim.player().unwrap().alive);
    }

    #[test]
    fn kill_wolf_grants_xp_and_loot() {
        let mut sim = Sim::new_combat_slice("Hero");
        let wolf_id = sim
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Mob)
            .unwrap()
            .id;
        let (wx, wz) = {
            let w = sim.entities.iter().find(|e| e.id == wolf_id).unwrap();
            (w.x, w.z)
        };
        if let Some(p) = sim.player_mut() {
            p.x = wx;
            p.z = wz;
            p.y = Entity::ground_at(p.x, p.z);
            p.resource = 100.0;
            p.target = Some(wolf_id);
            p.auto_attack = true;
        }

        let intent = PlayerIntent {
            move_x: 0.0,
            move_z: 0.0,
            facing: 0.0,
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
            if saw_kill && (sim.player_xp() > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1) {
                break;
            }
        }
        assert!(saw_kill, "expected a wolf kill within 400 ticks");
        assert!(
            sim.player_xp() > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1,
            "expected XP or level-up after kill"
        );
        assert!(
            saw_loot || sim.copper() > 0 || !sim.snapshot().inventory.is_empty(),
            "expected loot drop or auto-pickup"
        );
    }

    #[test]
    fn clear_target_intent_stops_auto_attack() {
        let mut sim = Sim::new_eastbrook("Clearer", PlayerClass::Warrior);
        let wolf_id = sim
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Mob && e.alive)
            .unwrap()
            .id;
        if let Some(p) = sim.player_mut() {
            p.target = Some(wolf_id);
            p.auto_attack = true;
        }

        let (snap, _) = sim.tick(PlayerIntent {
            clear_target: true,
            ..Default::default()
        });
        assert!(sim.player().unwrap().target.is_none());
        assert!(!sim.player().unwrap().auto_attack);
        assert!(snap.target_id.is_none());
        assert!(!snap.auto_attack);
    }

    #[test]
    fn snapshot_ability_bar_lists_class_kit() {
        let sim = Sim::new_eastbrook("Kit", PlayerClass::Warrior);
        let snap = sim.snapshot();
        assert!(
            snap.ability_bar.len() >= 3,
            "warrior kit should expose ≥3 slots"
        );
        assert_eq!(snap.ability_bar[0].slot, 1);
        assert_eq!(snap.ability_bar[0].ability_id, "heroic_strike");
        assert!(snap.ability_bar[0].known);
        assert_eq!(snap.ability_bar[1].ability_id, "cleave");
        assert!(!snap.ability_bar[1].known, "cleave gated above level 1");
        assert_eq!(snap.protocol_rev, PROTOCOL_REV);
    }

    #[test]
    fn death_release_spirit_respawns_at_eastbrook_graveyard() {
        let gy = woc_content::graveyard("eastbrook_graveyard").expect("eastbrook graveyard");
        let mut sim = Sim::new_eastbrook("Deadman", PlayerClass::Warrior);
        let pid = sim.player_id;
        let death_x = 22.0;
        let death_z = -20.0;
        if let Some(p) = sim.player_mut() {
            p.x = death_x;
            p.z = death_z;
            p.y = Entity::ground_at(p.x, p.z);
            p.hp = 0.0;
            p.auto_attack = true;
            p.target = Some(99);
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
        assert!(!sim.player().unwrap().alive);
        assert!(!sim.player().unwrap().auto_attack);

        // Dead player cannot deal damage even with attack intent.
        let wolf_id = sim
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Mob && e.alive)
            .map(|e| e.id);
        if let Some(wid) = wolf_id {
            let hp_before = sim.entities.iter().find(|e| e.id == wid).unwrap().hp;
            let intent = PlayerIntent {
                attack: true,
                ability: Some(AbilitySlot::Primary),
                target_id: Some(wid),
                ..Default::default()
            };
            let (_s, ev) = sim.tick(intent);
            let hp_after = sim.entities.iter().find(|e| e.id == wid).unwrap().hp;
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
        let p = sim.player().unwrap();
        assert!(p.alive);
        assert!((p.x - gy.x).abs() < 1e-5, "x {} vs gy {}", p.x, gy.x);
        assert!((p.z - gy.z).abs() < 1e-5, "z {} vs gy {}", p.z, gy.z);
        assert!((p.hp - p.hp_max).abs() < 1e-5);
        let snap = sim.snapshot_for_player(pid);
        assert!(!snap.is_dead);
    }

    #[test]
    fn death_release_spirit_is_deterministic() {
        let mut a = Sim::new_eastbrook("Twin", PlayerClass::Mage);
        let mut b = Sim::new_eastbrook("Twin", PlayerClass::Mage);
        for sim in [&mut a, &mut b] {
            if let Some(p) = sim.player_mut() {
                p.hp = 0.0;
            }
            let _ = sim.tick(PlayerIntent::default());
            assert!(sim.release_spirit(sim.player_id));
        }
        let pa = a.player().unwrap();
        let pb = b.player().unwrap();
        assert!((pa.x - pb.x).abs() < 1e-5);
        assert!((pa.z - pb.z).abs() < 1e-5);
        assert!((pa.hp - pb.hp).abs() < 1e-5);
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
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Mob && e.alive)
            .map(|e| e.id)
            .expect("mob");
        // Pull mob near player and pet into melee.
        let (px, pz) = {
            let p = sim.player().unwrap();
            (p.x, p.z)
        };
        if let Some(m) = sim.entities.iter_mut().find(|e| e.id == mob_id) {
            m.x = px + 1.5;
            m.z = pz;
        }
        sim.interact(pid, InteractAction::SummonPet);
        let pet_id = sim
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Pet)
            .map(|e| e.id)
            .unwrap();
        if let Some(pet) = sim.entities.iter_mut().find(|e| e.id == pet_id) {
            pet.x = px + 1.5;
            pet.z = pz;
            pet.swing_timer = 0.0;
        }
        let hp_before = sim.entities.iter().find(|e| e.id == mob_id).unwrap().hp;
        let intent = PlayerIntent {
            attack: true,
            target_id: Some(mob_id),
            ..Default::default()
        };
        for _ in 0..40 {
            let _ = sim.tick(intent);
        }
        let hp_after = sim.entities.iter().find(|e| e.id == mob_id).unwrap().hp;
        assert!(
            hp_after < hp_before,
            "expected pet+player damage ({hp_after} < {hp_before})"
        );
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
}
