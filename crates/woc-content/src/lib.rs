//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod classes;
pub mod delves;
pub mod dungeons;
pub mod gather_nodes;
pub mod graveyards;
pub mod items;
pub mod items_zone2;
pub mod mobs;
pub mod mobs_zone2;
pub mod npcs;
pub mod npcs_zone2;
pub mod pets;
pub mod professions;
pub mod quests;
pub mod quests_zone2;
pub mod recipes;
pub mod talents;
pub mod zone1;
pub mod zone2;

pub use abilities::{ability, AbilityDef, ABILITIES};
pub use classes::{
    class_ability_for_slot, class_def, known_abilities_at_level, ClassDef, ClassKitEntry,
    PlayerClass, ResourceType, CLASSES,
};
pub use delves::{delve, DelveDef, DelveReward, DelveRoomDef, DELVES};
pub use dungeons::{dungeon, DungeonDef, DUNGEONS};
pub use gather_nodes::{
    gather_node, gather_nodes_for_zone, GatherNodeDef, GATHER_NODES,
};
pub use graveyards::{graveyard, graveyard_for_zone, GraveyardDef, GRAVEYARDS};
pub use items::{item, ItemDef, ItemEquipSlot, ItemKind, ITEMS};
pub use items_zone2::ZONE2_ITEMS;
pub use mobs::{mob, LootEntry, MobTemplate, MOBS};
pub use mobs_zone2::ZONE2_MOBS;
pub use npcs::{npc, NpcDef, VendorOffer, NPCS};
pub use npcs_zone2::ZONE2_NPCS;
pub use pets::{pet, pet_for_class, PetDef, PETS};
pub use professions::{profession, ProfessionDef, ProfessionKind, PROFESSIONS};
pub use quests::{quest, QuestDef, QuestObjective, QuestReward, QUESTS};
pub use quests_zone2::ZONE2_QUESTS;
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
    fn every_class_has_multi_ability_kit() {
        assert_eq!(CLASSES.len(), 9);
        for class in CLASSES {
            assert!(
                class.kit.len() >= 3,
                "{} kit needs ≥3 abilities, got {}",
                class.name,
                class.kit.len()
            );
            let mut slots = Vec::new();
            for entry in class.kit {
                assert!(
                    (1..=5).contains(&entry.slot),
                    "{} kit slot {} out of 1..=5",
                    class.name,
                    entry.slot
                );
                assert!(
                    !slots.contains(&entry.slot),
                    "{} duplicate kit slot {}",
                    class.name,
                    entry.slot
                );
                slots.push(entry.slot);
                let abil = ability(entry.ability_id).unwrap_or_else(|| {
                    panic!("{} kit refs missing ability {}", class.name, entry.ability_id)
                });
                assert!(abil.min_level >= 1, "{} min_level", abil.id);
                if entry.slot == 1 {
                    assert_eq!(
                        entry.ability_id, class.primary_ability,
                        "{} slot 1 must match primary_ability",
                        class.name
                    );
                    assert_eq!(abil.min_level, 1, "{} primary must be level 1", class.name);
                }
            }
            assert!(
                slots.contains(&1),
                "{} kit must include slot 1 (primary)",
                class.name
            );
            assert!(
                class.kit.iter().any(|e| {
                    ability(e.ability_id).map(|a| a.min_level > 1).unwrap_or(false)
                }),
                "{} needs at least one level-gated ability",
                class.name
            );
        }
    }

    #[test]
    fn kit_slot_lookup_matches_keys() {
        for class in CLASSES {
            let primary = class_ability_for_slot(class.id, 1).expect("slot 1");
            assert_eq!(primary.id, class.primary_ability);
            assert!(class_ability_for_slot(class.id, 2).is_some());
            assert!(class_ability_for_slot(class.id, 3).is_some());
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
        assert!(
            graveyard("mirefen_graveyard").is_some(),
            "mirefen_graveyard id must resolve"
        );
        assert!(
            graveyard_for_zone("mirefen").is_some(),
            "graveyard_for_zone(mirefen) must resolve"
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
    fn talent_and_dungeon_lookups_are_safe() {
        assert!(!TALENTS.is_empty());
        assert!(!DUNGEONS.is_empty());
        assert!(talent("missing_talent").is_none());
        assert!(dungeon("eastbrook_crypt").is_some());
        assert!(dungeon("missing_dungeon").is_none());
    }

    #[test]
    fn hunter_and_warlock_have_pet_defs() {
        assert_eq!(PETS.len(), 2);
        let hunter = pet_for_class(PlayerClass::Hunter).expect("hunter pet");
        assert_eq!(hunter.id, "hunter_wolf");
        assert!(pet(hunter.id).is_some());
        let warlock = pet_for_class(PlayerClass::Warlock).expect("warlock pet");
        assert_eq!(warlock.id, "warlock_imp");
        assert!(pet_for_class(PlayerClass::Warrior).is_none());
        assert!(pet("missing_pet").is_none());
    }

    #[test]
    fn mirefen_layout_has_resolvable_content() {
        assert!(
            MIREFEN.npcs.len() >= 2,
            "mirefen needs ≥2 NPC spots, got {}",
            MIREFEN.npcs.len()
        );
        let mob_templates = MIREFEN
            .mobs
            .iter()
            .map(|spot| spot.mob_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            mob_templates.len() >= 3,
            "mirefen needs ≥3 distinct mob templates, got {}",
            mob_templates.len()
        );
        assert!(
            (MIREFEN.player_spawn_x != 0.0) || (MIREFEN.player_spawn_z != 0.0),
            "mirefen player spawn should be set"
        );

        for spot in MIREFEN.npcs {
            assert!(
                NPCS.iter().any(|npc| npc.id == spot.npc_id),
                "missing npc {}",
                spot.npc_id
            );
        }
        for spot in MIREFEN.mobs {
            assert!(
                MOBS.iter().any(|mob| mob.id == spot.mob_id),
                "missing mob {}",
                spot.mob_id
            );
        }
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
