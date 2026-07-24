//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod classes;
pub mod dungeons;
pub mod gather_nodes;
pub mod graveyards;
pub mod items;
pub mod mobs;
pub mod npcs;
pub mod professions;
pub mod quests;
pub mod recipes;
pub mod talents;
pub mod zone1;
pub mod zone2;

pub use abilities::{ability, AbilityDef, ABILITIES};
pub use classes::{class_def, ClassDef, PlayerClass, ResourceType, CLASSES};
pub use dungeons::{dungeon, DungeonDef, DUNGEONS};
pub use gather_nodes::{
    gather_node, gather_nodes_for_zone, GatherNodeDef, GATHER_NODES,
};
pub use graveyards::{graveyard, graveyard_for_zone, GraveyardDef, GRAVEYARDS};
pub use items::{item, ItemDef, ItemEquipSlot, ItemKind, ITEMS};
pub use mobs::{mob, LootEntry, MobTemplate, MOBS};
pub use npcs::{npc, NpcDef, VendorOffer, NPCS};
pub use professions::{profession, ProfessionDef, ProfessionKind, PROFESSIONS};
pub use quests::{quest, QuestDef, QuestObjective, QuestReward, QUESTS};
pub use recipes::{recipe, recipes_for_profession, RecipeDef, RecipeReagent, RECIPES};
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
    fn zone2_placeholders_have_empty_spots() {
        assert!(EASTFEN.npcs.is_empty());
        assert!(EASTFEN.mobs.is_empty());
        assert!(MIREFEN.npcs.is_empty());
        assert!(MIREFEN.mobs.is_empty());
    }

    #[test]
    fn professions_include_gathering_and_crafting() {
        assert!(
            PROFESSIONS
                .iter()
                .any(|p| p.kind == ProfessionKind::Gathering),
            "expected at least one gathering profession"
        );
        assert!(
            PROFESSIONS
                .iter()
                .any(|p| p.kind == ProfessionKind::Crafting),
            "expected at least one crafting profession"
        );
        assert!(profession("herbalism").is_some());
        assert!(profession("alchemy").is_some());
    }

    #[test]
    fn gather_nodes_resolve_profession_item_and_zone() {
        assert!(
            GATHER_NODES.len() >= 3,
            "expected ≥3 gather nodes, got {}",
            GATHER_NODES.len()
        );
        for node in GATHER_NODES {
            assert!(
                profession(node.profession_id).is_some(),
                "gather node {} missing profession {}",
                node.id,
                node.profession_id
            );
            assert!(
                ITEMS.iter().any(|i| i.id == node.item_id),
                "gather node {} missing item {}",
                node.id,
                node.item_id
            );
            assert!(
                KNOWN_ZONE_IDS.contains(&node.zone_id),
                "gather node {} references unknown zone_id {}",
                node.id,
                node.zone_id
            );
            assert!(node.count >= 1);
        }
        assert!(gather_node("eastbrook_meadow_silverleaf").is_some());
        assert!(gather_nodes_for_zone("eastbrook").count() >= 3);
    }

    #[test]
    fn recipes_consume_reagents_and_produce_items() {
        assert!(
            RECIPES.len() >= 2,
            "expected ≥2 recipes, got {}",
            RECIPES.len()
        );
        for r in RECIPES {
            assert!(
                profession(r.profession_id).is_some(),
                "recipe {} missing profession {}",
                r.id,
                r.profession_id
            );
            assert!(
                !r.reagents.is_empty(),
                "recipe {} must consume reagents",
                r.id
            );
            for reagent in r.reagents {
                assert!(
                    ITEMS.iter().any(|i| i.id == reagent.item_id),
                    "recipe {} missing reagent {}",
                    r.id,
                    reagent.item_id
                );
                assert!(reagent.count >= 1);
            }
            assert!(
                ITEMS.iter().any(|i| i.id == r.product_item_id),
                "recipe {} missing product {}",
                r.id,
                r.product_item_id
            );
            assert!(r.product_count >= 1);
        }
        assert!(recipe("minor_healing_salve").is_some());
        assert!(recipes_for_profession("alchemy").count() >= 2);
    }
}
