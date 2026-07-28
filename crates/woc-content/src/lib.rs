//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod classes;
pub mod items;
pub mod mobs;
pub mod npcs;
pub mod quests;
pub mod zone1;

pub use abilities::{ability, AbilityDef, ABILITIES};
pub use classes::{class_def, ClassDef, PlayerClass, ResourceType, CLASSES};
pub use items::{item, ItemDef, ItemKind, ITEMS};
pub use mobs::{mob, LootEntry, MobTemplate, MOBS};
pub use npcs::{npc, NpcDef, VendorOffer, NPCS};
pub use quests::{quest, QuestDef, QuestObjective, QuestReward, QUESTS};
pub use zone1::{MobSpot, NpcSpot, ZoneLayout, EASTBROOK};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_start_gear_exists() {
        assert_eq!(CLASSES.len(), 9);
        for class in CLASSES {
            assert!(
                ITEMS.iter().any(|i| i.id == class.start_weapon),
                "missing weapon {}",
                class.start_weapon
            );
            assert!(
                ITEMS.iter().any(|i| i.id == class.start_chest),
                "missing chest {}",
                class.start_chest
            );
            assert!(
                ABILITIES.iter().any(|a| a.id == class.primary_ability),
                "missing ability {}",
                class.primary_ability
            );
            for (item_id, _) in class.start_items {
                assert!(
                    ITEMS.iter().any(|i| i.id == *item_id),
                    "missing start item {item_id}"
                );
            }
        }
    }

    #[test]
    fn every_quest_npc_exists() {
        for q in QUESTS {
            assert!(
                NPCS.iter().any(|n| n.id == q.giver_npc),
                "missing giver {}",
                q.giver_npc
            );
            if let Some(turner) = q.turn_in_npc {
                assert!(
                    NPCS.iter().any(|n| n.id == turner),
                    "missing turn-in {turner}"
                );
            }
        }
    }

    #[test]
    fn eastbrook_spots_resolve() {
        for spot in EASTBROOK.npcs {
            assert!(
                NPCS.iter().any(|n| n.id == spot.npc_id),
                "missing npc {}",
                spot.npc_id
            );
        }
        for spot in EASTBROOK.mobs {
            assert!(
                MOBS.iter().any(|m| m.id == spot.mob_id),
                "missing mob {}",
                spot.mob_id
            );
        }
    }

    #[test]
    fn vendor_stock_items_exist() {
        for n in NPCS {
            for offer in n.vendor_stock {
                assert!(
                    ITEMS.iter().any(|i| i.id == offer.item_id),
                    "vendor {} missing item {}",
                    n.id,
                    offer.item_id
                );
            }
        }
    }
}
