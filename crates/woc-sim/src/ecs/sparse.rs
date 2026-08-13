//! Dense array + hashmap sparse index. Iteration is insertion order.

use woc_protocol::EntityId;

#[derive(Debug, Clone)]
pub struct SparseSet<T> {
    sparse: std::collections::HashMap<EntityId, usize>,
    dense_ids: Vec<EntityId>,
    dense: Vec<T>,
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            sparse: std::collections::HashMap::new(),
            dense_ids: Vec::new(),
            dense: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn contains(&self, id: EntityId) -> bool {
        self.sparse.contains_key(&id)
    }

    pub fn get(&self, id: EntityId) -> Option<&T> {
        let idx = *self.sparse.get(&id)?;
        self.dense.get(idx)
    }

    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut T> {
        let idx = *self.sparse.get(&id)?;
        self.dense.get_mut(idx)
    }

    /// Insert or overwrite. Returns the previous value if this id already had `T`.
    pub fn insert(&mut self, id: EntityId, value: T) -> Option<T> {
        if let Some(&idx) = self.sparse.get(&id) {
            let old = std::mem::replace(&mut self.dense[idx], value);
            return Some(old);
        }
        let idx = self.dense.len();
        self.sparse.insert(id, idx);
        self.dense_ids.push(id);
        self.dense.push(value);
        None
    }

    pub fn remove(&mut self, id: EntityId) -> Option<T> {
        let idx = self.sparse.remove(&id)?;
        let last = self.dense.len() - 1;
        let removed = self.dense.swap_remove(idx);
        self.dense_ids.swap_remove(idx);
        if idx < last {
            let swapped = self.dense_ids[idx];
            self.sparse.insert(swapped, idx);
        }
        Some(removed)
    }

    pub fn ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.dense_ids.iter().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &T)> + '_ {
        self.dense_ids.iter().copied().zip(self.dense.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntityId, &mut T)> + '_ {
        self.dense_ids.iter().copied().zip(self.dense.iter_mut())
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_overwrite_remove() {
        let mut s = SparseSet::new();
        assert!(s.insert(1, 10).is_none());
        assert_eq!(s.len(), 1);
        assert!(s.contains(1));
        assert_eq!(s.get(1), Some(&10));
        *s.get_mut(1).unwrap() = 10;
        assert_eq!(s.insert(1, 11), Some(10));
        assert_eq!(s.get(1), Some(&11));
        assert_eq!(s.remove(1), Some(11));
        assert!(s.get(1).is_none());
        assert!(s.is_empty());
    }

    #[test]
    fn iteration_is_insertion_order_not_id_order() {
        let mut s = SparseSet::new();
        s.insert(10, "c");
        s.insert(2, "a");
        s.insert(7, "b");
        let ids: Vec<_> = s.ids().collect();
        assert_eq!(ids, vec![10, 2, 7]);
        s.remove(2);
        let ids: Vec<_> = s.ids().collect();
        // swap-remove: last (7) moves into index of 2
        assert_eq!(ids, vec![10, 7]);
        for (_, v) in s.iter_mut() {
            *v = "x";
        }
        let pairs: Vec<_> = s.iter().map(|(id, v)| (id, *v)).collect();
        assert_eq!(pairs, vec![(10, "x"), (7, "x")]);
    }

    #[test]
    fn remove_last_does_not_corrupt_sparse() {
        let mut s = SparseSet::new();
        s.insert(1, 1);
        s.insert(2, 2);
        assert_eq!(s.remove(2), Some(2));
        assert_eq!(s.get(1), Some(&1));
        assert!(s.get(2).is_none());
    }
}
