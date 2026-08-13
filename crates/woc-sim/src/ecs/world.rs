//! Typed sparse-column world. Entity ids are monotonic and never reused.

use crate::ecs::components::{
    Auras, Bags, Bank, ClassKit, Combat, Component, Durable, Health, Home, Identity, InstanceAt,
    LootPile, LootTable, Motion, Owner, Progress, QuestLog, Respawn, Spirit, Threat, Transform,
};
use crate::ecs::SparseSet;
use woc_protocol::EntityId;

#[derive(Debug, Default)]
pub struct World {
    next_id: EntityId,
    live: Vec<EntityId>,
    live_index: std::collections::HashMap<EntityId, usize>,
    pub identity: SparseSet<Identity>,
    pub transform: SparseSet<Transform>,
    pub health: SparseSet<Health>,
    pub combat: SparseSet<Combat>,
    pub auras: SparseSet<Auras>,
    pub home: SparseSet<Home>,
    pub threat: SparseSet<Threat>,
    pub loot_table: SparseSet<LootTable>,
    pub respawn: SparseSet<Respawn>,
    pub loot_pile: SparseSet<LootPile>,
    pub owner: SparseSet<Owner>,
    pub class_kit: SparseSet<ClassKit>,
    pub bags: SparseSet<Bags>,
    pub quest_log: SparseSet<QuestLog>,
    pub progress: SparseSet<Progress>,
    pub bank: SparseSet<Bank>,
    pub motion: SparseSet<Motion>,
    pub spirit: SparseSet<Spirit>,
    pub instance_at: SparseSet<InstanceAt>,
    pub durable: SparseSet<Durable>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ..Self::default()
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        if id == 0 {
            return self.spawn();
        }
        let idx = self.live.len();
        self.live.push(id);
        self.live_index.insert(id, idx);
        id
    }

    /// Seed `next_id` when hydrating a realm that already assigned ids.
    pub fn set_next_id(&mut self, next: EntityId) {
        self.next_id = next.max(1);
    }

    pub fn next_id(&self) -> EntityId {
        self.next_id
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.live_index.contains_key(&id)
    }

    /// Register an existing id (dual-write from the fat `Entity` list). Does not allocate 0.
    pub fn adopt(&mut self, id: EntityId) {
        if id == 0 || self.contains(id) {
            return;
        }
        let idx = self.live.len();
        self.live.push(id);
        self.live_index.insert(id, idx);
        if id >= self.next_id {
            self.next_id = id.saturating_add(1);
        }
    }

    pub fn live_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.live.iter().copied()
    }

    pub fn spawn_count(&self) -> usize {
        self.live.len()
    }

    pub fn insert<C: Component>(&mut self, id: EntityId, value: C) {
        debug_assert!(self.contains(id), "insert on unknown entity {id}");
        C::storage_mut(self).insert(id, value);
    }

    pub fn get<C: Component>(&self, id: EntityId) -> Option<&C> {
        C::storage(self).get(id)
    }

    pub fn get_mut<C: Component>(&mut self, id: EntityId) -> Option<&mut C> {
        C::storage_mut(self).get_mut(id)
    }

    pub fn remove<C: Component>(&mut self, id: EntityId) -> Option<C> {
        C::storage_mut(self).remove(id)
    }

    pub fn ids<C: Component>(&self) -> Vec<EntityId> {
        C::storage(self).ids().collect()
    }

    pub fn despawn(&mut self, id: EntityId) -> bool {
        let Some(idx) = self.live_index.remove(&id) else {
            return false;
        };
        let last = self.live.len() - 1;
        self.live.swap_remove(idx);
        if idx < last {
            let swapped = self.live[idx];
            self.live_index.insert(swapped, idx);
        }
        self.clear_all_columns(id);
        true
    }

    fn clear_all_columns(&mut self, id: EntityId) {
        self.identity.remove(id);
        self.transform.remove(id);
        self.health.remove(id);
        self.combat.remove(id);
        self.auras.remove(id);
        self.home.remove(id);
        self.threat.remove(id);
        self.loot_table.remove(id);
        self.respawn.remove(id);
        self.loot_pile.remove(id);
        self.owner.remove(id);
        self.class_kit.remove(id);
        self.bags.remove(id);
        self.quest_log.remove(id);
        self.progress.remove(id);
        self.bank.remove(id);
        self.motion.remove(id);
        self.spirit.remove(id);
        self.instance_at.remove(id);
        self.durable.remove(id);
    }
}
