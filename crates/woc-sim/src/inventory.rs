use crate::content::enchants::EnchantId;
use crate::item::{EquipSlot, ItemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemInstance {
    pub id: u64,
    pub item: ItemId,
    pub enchant: Option<EnchantId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryError {
    Full,
    Missing,
}

#[derive(Clone, Debug)]
pub struct Inventory {
    slots: Vec<Option<ItemStack>>,
    pub instances: Vec<ItemInstance>,
    next_instance_id: u64,
}

impl Inventory {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            slots: vec![None; cap],
            instances: Vec::new(),
            next_instance_id: 1,
        }
    }

    pub fn instance(&self, id: u64) -> Option<&ItemInstance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn instance_mut(&mut self, id: u64) -> Option<&mut ItemInstance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    fn is_instanced_equipment(item: ItemId) -> bool {
        let def = crate::content::items::item_def(item);
        !def.stackable && def.slot != EquipSlot::None
    }

    fn occupied_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count() + self.instances.len()
    }

    fn free_slots(&self) -> usize {
        self.slots.len().saturating_sub(self.occupied_slots())
    }

    pub fn count(&self, item: ItemId) -> u16 {
        let stacked: u16 = self
            .slots
            .iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum();
        let instanced = self.instances.iter().filter(|i| i.item == item).count() as u16;
        stacked.saturating_add(instanced)
    }

    pub fn try_add(&mut self, stack: ItemStack) -> Result<(), InventoryError> {
        let def = crate::content::items::item_def(stack.item);
        if Self::is_instanced_equipment(stack.item) {
            if self.free_slots() < usize::from(stack.count) {
                return Err(InventoryError::Full);
            }
            for _ in 0..stack.count {
                let id = self.next_instance_id;
                self.next_instance_id += 1;
                self.instances.push(ItemInstance {
                    id,
                    item: stack.item,
                    enchant: None,
                });
            }
            return Ok(());
        }
        if def.stackable {
            if let Some(existing) = self
                .slots
                .iter_mut()
                .flatten()
                .find(|s| s.item == stack.item)
            {
                existing.count = existing.count.saturating_add(stack.count);
                return Ok(());
            }
        }
        if self.free_slots() == 0 {
            return Err(InventoryError::Full);
        }
        let empty = self
            .slots
            .iter_mut()
            .find(|s| s.is_none())
            .ok_or(InventoryError::Full)?;
        *empty = Some(stack);
        Ok(())
    }

    pub fn try_remove(&mut self, item: ItemId, count: u16) -> Result<(), InventoryError> {
        if self.count(item) < count {
            return Err(InventoryError::Missing);
        }
        let mut remaining = count;
        for slot in self.slots.iter_mut() {
            if remaining == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item == item {
                    let take = stack.count.min(remaining);
                    stack.count -= take;
                    remaining -= take;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        while remaining > 0 && Self::is_instanced_equipment(item) {
            let index = self
                .instances
                .iter()
                .position(|instance| instance.item == item)
                .ok_or(InventoryError::Missing)?;
            self.instances.remove(index);
            remaining -= 1;
        }
        if remaining > 0 {
            return Err(InventoryError::Missing);
        }
        Ok(())
    }

    pub fn has(&self, item: ItemId) -> bool {
        self.count(item) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stackable_ore_merges_then_removes() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 2,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 3,
        })
        .unwrap();
        assert_eq!(inv.count(ItemId::CopperOre), 5);
        inv.try_remove(ItemId::CopperOre, 5).unwrap();
        assert_eq!(inv.count(ItemId::CopperOre), 0);
    }

    #[test]
    fn full_bag_rejects_unstackable_tools() {
        let mut inv = Inventory::with_capacity(1);
        inv.try_add(ItemStack {
            item: ItemId::CopperPick,
            count: 1,
        })
        .unwrap();
        let err = inv
            .try_add(ItemStack {
                item: ItemId::CopperSickle,
                count: 1,
            })
            .unwrap_err();
        assert_eq!(err, InventoryError::Full);
    }

    #[test]
    fn full_bag_rejects_additional_instanced_equipment() {
        let mut inv = Inventory::with_capacity(1);
        inv.try_add(ItemStack {
            item: ItemId::CopperShortsword,
            count: 1,
        })
        .unwrap();

        let err = inv
            .try_add(ItemStack {
                item: ItemId::CopperChainVest,
                count: 1,
            })
            .unwrap_err();

        assert_eq!(err, InventoryError::Full);
        assert_eq!(inv.count(ItemId::CopperShortsword), 1);
        assert_eq!(inv.count(ItemId::CopperChainVest), 0);
    }

    #[test]
    fn try_remove_removes_instanced_equipment() {
        let mut inv = Inventory::with_capacity(1);
        inv.try_add(ItemStack {
            item: ItemId::CopperShortsword,
            count: 1,
        })
        .unwrap();

        inv.try_remove(ItemId::CopperShortsword, 1).unwrap();

        assert_eq!(inv.count(ItemId::CopperShortsword), 0);
        assert!(inv.instances.is_empty());
    }
}
