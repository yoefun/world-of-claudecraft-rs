//! Sim coordinator: tick loop, intents, snapshots.

use crate::combat::{
    collect_pending_mob_kills, grant_xp, spawn_wolf_loot, try_pickup_loot, update_mob_combat,
    update_player_combat,
};
use crate::entity::{create_warrior, create_wolf, Entity};
use crate::mob::update_mob_ai;
use crate::player_motion::step_player_motion;
use crate::rng::Rng;
use crate::types::{xp_to_next, HEROIC_STRIKE_CD, WOLF_XP};
use crate::world::WORLD_SEED;
use woc_protocol::{
    AbilitySlot, EntityId, EntityKind, EntitySnapshot, PlayerIntent, PlayerProgress, SimEvent,
    TickSnapshot,
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
    pub bag_item: Option<String>,
    pub events: Vec<SimEvent>,
}

impl Sim {
    /// Start a new offline combat-slice world with a Warrior near Eastbrook spawn.
    pub fn new_combat_slice(player_name: &str) -> Self {
        let seed = WORLD_SEED;
        let mut rng = Rng::new(seed);
        let mut next_id = 1u32;
        let mut entities = Vec::new();

        let player_id = next_id;
        next_id += 1;
        // Spawn near origin (town flat).
        entities.push(create_warrior(player_id, player_name, 2.0, 4.0));

        // Wolf camp north of spawn.
        let wolf_spots = [
            (-8.0_f32, -22.0_f32, "Young Wolf"),
            (-4.0, -26.0, "Young Wolf"),
            (2.0, -24.0, "Young Wolf"),
            (6.0, -28.0, "Scarred Wolf"),
        ];
        for (x, z, name) in wolf_spots {
            let id = next_id;
            next_id += 1;
            let mut wolf = create_wolf(id, name, x, z, WOLF_XP);
            // Tiny jitter so packs don't stack perfectly.
            wolf.x += (rng.next_f32() - 0.5) * 1.5;
            wolf.z += (rng.next_f32() - 0.5) * 1.5;
            wolf.home_x = wolf.x;
            wolf.home_z = wolf.z;
            wolf.y = Entity::ground_at(wolf.x, wolf.z);
            entities.push(wolf);
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
            bag_item: None,
            events: Vec::new(),
        }
    }

    pub fn player(&self) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == self.player_id)
    }

    pub fn player_mut(&mut self) -> Option<&mut Entity> {
        let id = self.player_id;
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn tick(&mut self, intent: PlayerIntent) -> (TickSnapshot, Vec<SimEvent>) {
        self.events.clear();
        self.tick += 1;

        // Apply intent to player.
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
                        // Auto-acquire nearest living mob in range.
                        if let Some(tid) = nearest_mob(&self.entities, &self.entities[pi], 30.0) {
                            self.entities[pi].target = Some(tid);
                        }
                    }
                }
            }
        }

        // Mob AI.
        let mob_ids: Vec<EntityId> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob && e.alive)
            .map(|e| e.id)
            .collect();
        for mid in &mob_ids {
            update_mob_ai(*mid, self.player_id, &mut self.entities);
        }

        // Combat.
        update_player_combat(
            self.player_id,
            &mut self.entities,
            intent.ability,
            &mut self.events,
        );
        for mid in &mob_ids {
            update_mob_combat(*mid, self.player_id, &mut self.entities, &mut self.events);
        }

        // Rewards for kills that happened this tick.
        let rewards = collect_pending_mob_kills(&self.events, &self.entities);
        for reward in rewards {
            if let Some(pi) = self.entities.iter().position(|e| e.id == self.player_id) {
                let mut xp = self.player_xp;
                grant_xp(&mut self.entities[pi], &mut xp, reward.xp, &mut self.events);
                self.player_xp = xp;
            }
            spawn_wolf_loot(
                &mut self.next_id,
                &mut self.entities,
                &mut self.rng,
                reward.x,
                reward.z,
            );
        }

        // Loot pickup.
        {
            let mut copper = self.copper;
            let mut bag = self.bag_item.clone();
            try_pickup_loot(
                self.player_id,
                &mut self.entities,
                &mut copper,
                &mut bag,
                &mut self.events,
            );
            self.copper = copper;
            self.bag_item = bag;
        }

        // Prune dead loot after a while? keep corpses as dead entities for snapshot filter.
        let snapshot = self.snapshot();
        let events = self.events.clone();
        (snapshot, events)
    }

    pub fn snapshot(&self) -> TickSnapshot {
        let player = self.player();
        let level = player.map(|p| p.level).unwrap_or(1);
        let target_id = player.and_then(|p| p.target);
        let ability_cd = player.map(|p| p.ability_cd).unwrap_or(0.0);
        let entities = self
            .entities
            .iter()
            .filter(|e| e.alive || e.kind == EntityKind::Mob)
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
                bag_item: self.bag_item.clone(),
            },
            target_id,
            ability_ready: ability_cd <= 0.0,
            ability_cooldown: ability_cd / HEROIC_STRIKE_CD,
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

/// Helper for tests: drive intents until a condition or tick cap.
pub fn run_until<F>(sim: &mut Sim, mut intent: PlayerIntent, max_ticks: u64, mut pred: F) -> bool
where
    F: FnMut(&Sim, &[SimEvent]) -> bool,
{
    for _ in 0..max_ticks {
        let (_snap, events) = sim.tick(intent);
        if pred(sim, &events) {
            return true;
        }
        // Keep attacking once engaged.
        intent.attack = true;
        intent.ability = Some(AbilitySlot::Primary);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::AbilitySlot;

    #[test]
    fn combat_slice_spawns_wolves() {
        let sim = Sim::new_combat_slice("Test");
        let wolves = sim
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Mob)
            .count();
        assert_eq!(wolves, 4);
        assert!(sim.player().unwrap().alive);
    }

    #[test]
    fn kill_wolf_grants_xp_and_loot() {
        let mut sim = Sim::new_combat_slice("Hero");
        // Place player on top of first wolf and smash.
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
            if saw_kill
                && (sim.player_xp > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1)
                && saw_loot
            {
                break;
            }
        }
        assert!(saw_kill, "expected a wolf kill within 400 ticks");
        assert!(
            sim.player_xp > 0 || sim.player().map(|p| p.level).unwrap_or(1) > 1,
            "expected XP or level-up after kill"
        );
        assert!(
            saw_loot || sim.copper > 0 || sim.bag_item.is_some(),
            "expected loot drop or auto-pickup"
        );
        assert!(sim.player_xp >= WOLF_XP || sim.player().unwrap().level > 1);
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
