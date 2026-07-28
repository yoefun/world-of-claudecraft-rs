//! Sim coordinator: tick loop, intents, snapshots.

use crate::combat::{
    collect_pending_mob_kills, grant_xp, spawn_mob_loot, try_pickup_loot, update_mob_combat,
    update_player_combat,
};
use crate::entity::{
    create_mob_from_template, create_npc_from_template, create_player, Entity, QuestState,
};
use crate::interaction::vendor_snapshot;
use crate::mob::update_mob_ai;
use crate::player_motion::step_player_motion;
use crate::quests::on_mob_killed;
use crate::rng::Rng;
use crate::types::xp_to_next;
use crate::world::WORLD_SEED;
use woc_content::{ability, class_def, PlayerClass, EASTBROOK};
use woc_protocol::{
    EntityId, EntityKind, EntitySnapshot, EquipmentSnapshot, InteractAction, InvSlotSnapshot,
    PlayerIntent, PlayerProgress, QuestLogEntry, SimEvent, TickSnapshot, PROTOCOL_REV,
};

pub struct Sim {
    pub tick: u64,
    pub seed: u32,
    pub rng: Rng,
    pub entities: Vec<Entity>,
    pub next_id: EntityId,
    pub player_id: EntityId,
    pub player_xp: u32,
    pub copper: u32,
    pub events: Vec<SimEvent>,
    pub pending_intent: PlayerIntent,
    pub class: PlayerClass,
}

impl Sim {
    /// Start Eastbrook from content tables.
    pub fn new_eastbrook(player_name: &str, class: PlayerClass) -> Self {
        let seed = WORLD_SEED;
        let mut rng = Rng::new(seed);
        let mut next_id = 1u32;
        let mut entities = Vec::new();

        let player_id = next_id;
        next_id += 1;
        entities.push(create_player(
            player_id,
            player_name,
            class,
            EASTBROOK.player_spawn_x,
            EASTBROOK.player_spawn_z,
        ));

        for spot in EASTBROOK.npcs {
            let id = next_id;
            next_id += 1;
            if let Some(npc) = create_npc_from_template(id, spot.npc_id, spot.x, spot.z) {
                entities.push(npc);
            }
        }

        for spot in EASTBROOK.mobs {
            let id = next_id;
            next_id += 1;
            if let Some(mut mob) = create_mob_from_template(id, spot.mob_id, spot.x, spot.z) {
                mob.x += (rng.next_f32() - 0.5) * 1.5;
                mob.z += (rng.next_f32() - 0.5) * 1.5;
                mob.home_x = mob.x;
                mob.home_z = mob.z;
                mob.y = Entity::ground_at(mob.x, mob.z);
                entities.push(mob);
            }
        }

        Self {
            tick: 0,
            seed,
            rng,
            entities,
            next_id,
            player_id,
            player_xp: 0,
            copper: 0,
            events: Vec::new(),
            pending_intent: PlayerIntent::default(),
            class,
        }
    }

    /// Backward-compatible Warrior Eastbrook spawn.
    pub fn new_combat_slice(player_name: &str) -> Self {
        Self::new_eastbrook(player_name, PlayerClass::Warrior)
    }

    pub fn player(&self) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == self.player_id)
    }

    pub fn player_mut(&mut self) -> Option<&mut Entity> {
        let id = self.player_id;
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn interact(&mut self, target_id: EntityId, action: InteractAction) {
        woc_protocol::WorldHost::interact(self, self.player_id, target_id, action);
    }

    pub fn grant_item(
        &mut self,
        player_id: EntityId,
        item_id: &str,
        count: u32,
    ) -> Result<(), &'static str> {
        let Some(p) = self.entities.iter_mut().find(|e| e.id == player_id) else {
            return Err("no player");
        };
        crate::inventory::grant_item(p, item_id, count, &mut self.events)?;
        crate::quests::on_inventory_changed(p, &mut self.events);
        Ok(())
    }

    pub fn tick(&mut self, intent: PlayerIntent) -> (TickSnapshot, Vec<SimEvent>) {
        self.events.clear();
        self.pending_intent = intent;
        self.tick += 1;

        // Phase 1–2: apply intent + motion
        if let Some(pi) = self.entities.iter().position(|e| e.id == self.player_id) {
            if self.entities[pi].alive {
                step_player_motion(
                    &mut self.entities[pi],
                    intent.move_x,
                    intent.move_z,
                    intent.facing,
                );
                if let Some(tid) = intent.target_id {
                    self.entities[pi].target = Some(tid);
                }
                if intent.attack {
                    self.entities[pi].auto_attack = true;
                    if self.entities[pi].target.is_none() {
                        if let Some(tid) = nearest_mob(&self.entities, &self.entities[pi], 30.0) {
                            self.entities[pi].target = Some(tid);
                        }
                    }
                }
            }
        }

        // Phase 3: player combat
        update_player_combat(
            self.player_id,
            &mut self.entities,
            intent.ability,
            &mut self.events,
        );

        // Phase 4: mob AI + combat
        let mob_ids: Vec<EntityId> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob && e.alive)
            .map(|e| e.id)
            .collect();
        for mid in &mob_ids {
            update_mob_ai(*mid, self.player_id, &mut self.entities);
        }
        for mid in &mob_ids {
            update_mob_combat(*mid, self.player_id, &mut self.entities, &mut self.events);
        }

        // Rewards for kills
        let rewards = collect_pending_mob_kills(&self.events, &self.entities);
        for reward in rewards {
            if let Some(pi) = self.entities.iter().position(|e| e.id == self.player_id) {
                let mut xp = self.player_xp;
                grant_xp(&mut self.entities[pi], &mut xp, reward.xp, &mut self.events);
                self.player_xp = xp;
                if let Some(ref tid) = reward.template_id {
                    on_mob_killed(&mut self.entities[pi], tid, &mut self.events);
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

        // Phase 6: loot pickup
        {
            let mut copper = self.copper;
            try_pickup_loot(
                self.player_id,
                &mut self.entities,
                &mut copper,
                &mut self.events,
            );
            self.copper = copper;
        }

        let snapshot = self.snapshot();
        let events = self.events.clone();
        (snapshot, events)
    }

    pub fn snapshot(&self) -> TickSnapshot {
        let player = self.player();
        let level = player.map(|p| p.level).unwrap_or(1);
        let target_id = player.and_then(|p| p.target);
        let ability_cd = player.map(|p| p.ability_cd).unwrap_or(0.0);
        let ability_name = player
            .and_then(|p| p.primary_ability.as_deref())
            .and_then(ability)
            .map(|a| a.name.to_string())
            .unwrap_or_default();
        let cd_max = player
            .and_then(|p| p.primary_ability.as_deref())
            .and_then(ability)
            .map(|a| a.cooldown)
            .unwrap_or(3.0);
        let class_id = self.class.as_str().to_string();
        let resource_type = class_def(self.class).resource_type;
        let resource_type = match resource_type {
            woc_content::ResourceType::Rage => "rage",
            woc_content::ResourceType::Mana => "mana",
            woc_content::ResourceType::Energy => "energy",
        }
        .to_string();

        let inventory = player
            .map(|p| {
                p.inventory
                    .iter()
                    .filter_map(|s| {
                        s.as_ref().map(|st| InvSlotSnapshot {
                            item_id: st.item_id.clone(),
                            count: st.count,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let equipment = player
            .map(|p| EquipmentSnapshot {
                main_hand: p.equipment.main_hand.clone(),
                off_hand: p.equipment.off_hand.clone(),
                chest: p.equipment.chest.clone(),
            })
            .unwrap_or_default();

        let quest_log = player
            .map(|p| {
                p.quest_log
                    .iter()
                    .map(|q| QuestLogEntry {
                        quest_id: q.quest_id.clone(),
                        state: match q.state {
                            QuestState::Active => "active",
                            QuestState::Ready => "ready",
                            QuestState::Completed => "completed",
                        }
                        .to_string(),
                        counts: q.counts.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let open_vendor = player.and_then(|p| vendor_snapshot(&self.entities, p));

        let entities = self
            .entities
            .iter()
            .filter(|e| e.alive || e.kind == EntityKind::Mob || e.kind == EntityKind::Npc)
            .map(|e| EntitySnapshot {
                id: e.id,
                kind: e.kind,
                x: e.x,
                y: e.y,
                z: e.z,
                yaw: e.yaw,
                hp: e.hp,
                hp_max: e.hp_max,
                level: e.level,
                name: e.name.clone(),
                resource: e.resource,
                resource_max: e.resource_max,
                alive: e.alive,
                template_id: e.template_id.clone(),
            })
            .collect();

        TickSnapshot {
            tick: self.tick,
            player_id: self.player_id,
            entities,
            progress: PlayerProgress {
                xp: self.player_xp,
                xp_to_level: xp_to_next(level),
                level,
                copper: self.copper,
                bag_item: None,
                class_id,
                resource_type,
            },
            target_id,
            ability_ready: ability_cd <= 0.0,
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
        }
    }
}

fn nearest_mob(entities: &[Entity], from: &Entity, max_range: f32) -> Option<EntityId> {
    let mut best: Option<(EntityId, f32)> = None;
    for e in entities {
        if e.kind != EntityKind::Mob || !e.alive {
            continue;
        }
        let dx = e.x - from.x;
        let dz = e.z - from.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d > max_range {
            continue;
        }
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((e.id, d));
        }
    }
    best.map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::{AbilitySlot, InteractAction, WorldHost};

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
        let snap = sim.snapshot_for(sim.player_id);
        assert!(snap
            .inventory
            .iter()
            .any(|s| s.item_id == "wolf_fang" && s.count == 1));
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

        // Kill three young wolves by dealing lethal damage.
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

        // Return to quest giver for turn-in.
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
        let log = sim.snapshot_for(sim.player_id).quest_log;
        assert!(log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == "completed"));
    }

    #[test]
    fn vendor_buy_spend_copper() {
        let mut sim = Sim::new_eastbrook("V", PlayerClass::Warrior);
        sim.copper = 100;
        let vendor = sim
            .entities
            .iter()
            .find(|e| e.template_id.as_deref() == Some("trader_wilkes"))
            .unwrap()
            .id;
        // Move player next to vendor.
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
        assert!(sim.copper < 100);
        assert!(sim
            .snapshot_for(sim.player_id)
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
            if saw_kill && (sim.player_xp > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1) {
                break;
            }
        }
        assert!(saw_kill, "expected a wolf kill within 400 ticks");
        assert!(
            sim.player_xp > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1,
            "expected XP or level-up after kill"
        );
        assert!(
            saw_loot || sim.copper > 0 || !sim.snapshot().inventory.is_empty(),
            "expected loot drop or auto-pickup"
        );
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
}
