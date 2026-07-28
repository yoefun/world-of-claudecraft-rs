//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod classes;
pub mod dungeons;
pub mod graveyards;
pub mod items;
pub mod items_zone2;
pub mod mobs;
pub mod mobs_zone2;
pub mod npcs;
pub mod npcs_zone2;
pub mod quests;
pub mod quests_zone2;
pub mod talents;
pub mod zone1;
pub mod zone2;

pub use abilities::{ability, AbilityDef, ABILITIES};
pub use classes::{class_def, ClassDef, PlayerClass, ResourceType, CLASSES};
pub use dungeons::{dungeon, DungeonDef, DUNGEONS};
pub use graveyards::{graveyard, graveyard_for_zone, GraveyardDef, GRAVEYARDS};
pub use items::{item, ItemDef, ItemEquipSlot, ItemKind, ITEMS};
pub use items_zone2::ZONE2_ITEMS;
pub use mobs::{mob, LootEntry, MobTemplate, MOBS};
pub use mobs_zone2::ZONE2_MOBS;
pub use npcs::{npc, NpcDef, VendorOffer, NPCS};
pub use npcs_zone2::ZONE2_NPCS;
pub use quests::{quest, QuestDef, QuestObjective, QuestReward, QUESTS};
pub use quests_zone2::ZONE2_QUESTS;
pub use talents::{talent, TalentDef, TALENTS};
pub use zone1::{MobSpot, NpcSpot, ZoneLayout, EASTBROOK};
pub use zone2::{EASTFEN, MIREFEN};

/// Known zone id strings referenced by graveyards and future zone tables.
pub const KNOWN_ZONE_IDS: &[&str] = &["eastbrook", "eastfen", "mirefen"];

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
        for q in QUESTS.iter() {
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
    fn every_quest_objective_refs_exist() {
        for q in QUESTS.iter() {
            for obj in q.objectives {
                match obj {
                    QuestObjective::Kill { mob_id, .. } => {
                        assert!(
                            MOBS.iter().any(|m| m.id == *mob_id),
                            "quest {} missing mob {mob_id}",
                            q.id
                        );
                    }
                    QuestObjective::Collect { item_id, .. } => {
                        assert!(
                            ITEMS.iter().any(|i| i.id == *item_id),
                            "quest {} missing item {item_id}",
                            q.id
                        );
                    }
                    QuestObjective::Talk { npc_id, .. } => {
                        assert!(
                            NPCS.iter().any(|n| n.id == *npc_id),
                            "quest {} missing talk npc {npc_id}",
                            q.id
                        );
                    }
                }
            }
            if let Some(item_id) = q.reward.item_id {
                assert!(
                    ITEMS.iter().any(|i| i.id == item_id),
                    "quest {} missing reward item {item_id}",
                    q.id
                );
            }
        }
    }

    #[test]
    fn every_mob_loot_item_exists() {
        for m in MOBS.iter() {
            for entry in m.loot {
                assert!(
                    ITEMS.iter().any(|i| i.id == entry.item_id),
                    "mob {} loot missing item {}",
                    m.id,
                    entry.item_id
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
    fn eastfen_layout_meets_zone2_dod() {
        assert!(
            EASTFEN.npcs.len() >= 3,
            "eastfen needs ≥3 NPC spots, got {}",
            EASTFEN.npcs.len()
        );
        let crawler_spots = EASTFEN
            .mobs
            .iter()
            .filter(|s| s.mob_id == "fen_crawler")
            .count();
        let toad_spots = EASTFEN
            .mobs
            .iter()
            .filter(|s| s.mob_id == "mire_toad")
            .count();
        assert!(crawler_spots >= 2, "fen crawler camp too thin");
        assert!(toad_spots >= 2, "mire toad camp too thin");
        assert!(
            (EASTFEN.player_spawn_x != 0.0) || (EASTFEN.player_spawn_z != 0.0),
            "eastfen player spawn should be set"
        );

        for spot in EASTFEN.npcs {
            assert!(
                NPCS.iter().any(|n| n.id == spot.npc_id),
                "missing npc {}",
                spot.npc_id
            );
        }
        for spot in EASTFEN.mobs {
            assert!(
                MOBS.iter().any(|m| m.id == spot.mob_id),
                "missing mob {}",
                spot.mob_id
            );
        }
    }

    #[test]
    fn zone2_content_volume() {
        assert!(
            ZONE2_QUESTS.len() >= 5,
            "need ≥5 zone2 quests, got {}",
            ZONE2_QUESTS.len()
        );
        assert!(
            ZONE2_MOBS.len() >= 3,
            "need ≥3 zone2 mob templates, got {}",
            ZONE2_MOBS.len()
        );
        assert!(ZONE2_NPCS.len() >= 3);
        assert!(!ZONE2_ITEMS.is_empty());
    }

    #[test]
    fn vendor_stock_items_exist() {
        for n in NPCS.iter() {
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

    #[test]
    fn graveyards_reference_known_zone_ids() {
        assert!(!GRAVEYARDS.is_empty());
        assert!(
            GRAVEYARDS.iter().any(|g| g.zone_id == "eastbrook"),
            "expected an Eastbrook graveyard entry"
        );
        assert!(
            graveyard("eastbrook_graveyard").is_some(),
            "eastbrook_graveyard id must resolve"
        );
        assert!(
            graveyard_for_zone("eastbrook").is_some(),
            "graveyard_for_zone(eastbrook) must resolve"
        );
        for g in GRAVEYARDS {
            assert!(
                KNOWN_ZONE_IDS.contains(&g.zone_id),
                "graveyard {} references unknown zone_id {}",
                g.id,
                g.zone_id
            );
        }
    }

    #[test]
    fn empty_talent_and_dungeon_lookups_are_safe() {
        assert!(TALENTS.is_empty());
        assert!(DUNGEONS.is_empty());
        assert!(talent("missing_talent").is_none());
        assert!(dungeon("missing_dungeon").is_none());
    }

    #[test]
    fn mirefen_remains_placeholder() {
        assert!(MIREFEN.npcs.is_empty());
        assert!(MIREFEN.mobs.is_empty());
    }
}
