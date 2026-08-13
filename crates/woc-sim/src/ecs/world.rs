//! Typed sparse-column world. Entity ids are monotonic and never reused.

use crate::ecs::components::{Component, Identity, Transform};
use crate::ecs::SparseSet;
use woc_protocol::EntityId;

#[derive(Debug, Default)]
pub struct World {
    next_id: EntityId,
    live: Vec<EntityId>,
    live_index: std::collections::HashMap<EntityId, usize>,
    pub identity: SparseSet<Identity>,
    pub transform: SparseSet<Transform>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
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
    }
}
