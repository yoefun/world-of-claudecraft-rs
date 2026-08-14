//! Authoritative game content tables for the Rust rewrite.
//! Pure data: no Bevy, no networking, no wall clock.

pub mod abilities;
pub mod ability_effects;
pub mod classes;
pub mod delves;
pub mod dungeons;
pub mod enchants;
pub mod factions;
pub mod gather_nodes;
pub mod graveyards;
pub mod items;
pub mod items_zone2;
pub mod mobs;
pub mod mobs_zone2;
pub mod mobs_zone3;
pub mod mounts;
pub mod npcs;
pub mod npcs_zone2;
pub mod npcs_zone3;
pub mod pets;
pub mod professions;
pub mod quests;
pub mod quests_zone2;
pub mod quests_zone3;
pub mod recipes;
pub mod stations;
pub mod talents;
pub mod world_spatial;
pub mod zone1;
pub mod zone2;
pub mod zone3;

pub use abilities::{ability, aura_for_ability, AbilityDef, ABILITIES};
pub use ability_effects::{aura, AbilityEffect, AbilityFlags, AuraDef, DamageSchool, AURAS};
pub use classes::{
    class_ability_for_slot, class_def, known_abilities_at_level, ClassDef, ClassKitEntry,
    PlayerClass, ResourceType, CLASSES,
};
pub use delves::{delve, DelveDef, DelveReward, DelveRoomDef, DELVES};
pub use dungeons::{dungeon, DungeonDef, DungeonTrashSpot, DUNGEONS};
pub use enchants::{
    disenchant_yield, profession_enchant, EnchantReagent, ProfessionEnchantDef, PROFESSION_ENCHANTS,
};
pub use factions::{
    clamp_reputation, discounted_price, faction, standing_at, standing_from_value, standing_next,
    vendor_discount_pct, FactionDef, RepAward, Standing, EXALTED_AT, FACTIONS,
    FACTION_EASTBROOK_WATCH, FACTION_EASTFEN_CIRCLE, FACTION_HIGHWATCH, FACTION_MIREFEN_FERRY,
    FRIENDLY_AT, HONORED_AT, STANDING_CAP, STANDING_FLOOR,
};
pub use gather_nodes::{gather_node, gather_nodes_for_zone, GatherNodeDef, GATHER_NODES};
pub use graveyards::{graveyard, graveyard_for_zone, GraveyardDef, GRAVEYARDS};
pub use items::{
    base_of, can_dual_wield, can_equip, class_armor_cap, enchant, fine_substitute_for, item,
    item_is_gathered, quality_mult, reagent_unit_value, ArmorClass, EnchantDef, EquipDeny,
    ItemBind, ItemDef, ItemEquipSlot, ItemKind, ItemQuality, WeaponStyle, ENCHANTS, ITEMS,
};
pub use items_zone2::ZONE2_ITEMS;
pub use mobs::{mob, LootEntry, MobTemplate, MOBS};
pub use mobs_zone2::ZONE2_MOBS;
pub use mobs_zone3::ZONE3_MOBS;
pub use mounts::{
    mount, mount_by_item, riding_rank, riding_rank_by_n, MountDef, MountKind, RidingRankDef,
    MOUNTS, RIDING_RANKS,
};
pub use npcs::{npc, NpcDef, NpcService, VendorOffer, NPCS};
pub use npcs_zone2::ZONE2_NPCS;
pub use npcs_zone3::ZONE3_NPCS;
pub use pets::{pet, pet_for_class, PetDef, PETS};
pub use professions::{
    gathering_tool_item, mob_is_skinnable, profession, ProfessionDef, ProfessionKind, PROFESSIONS,
};
pub use quests::{
    quest, QuestDef, QuestObjective, QuestRepeat, QuestReward, DAILY_PERIOD_TICKS, QUESTS,
};
pub use quests_zone2::ZONE2_QUESTS;
pub use quests_zone3::ZONE3_QUESTS;
pub use recipes::{
    craft_fee, recipe, recipes_for_profession, RecipeDef, RecipeReagent, CRAFT_BATCH_MAX, RECIPES,
};
pub use stations::{in_station_range, station, StationDef, STATIONS, STATION_RADIUS};
pub use talents::{
    format_talent_effect, points_spent_below_tier, talent, talent_tier_unlocked, TalentDef,
    POINTS_PER_TIER, TALENTS,
};
pub use world_spatial::{
    canonical_zone_id, zone_at, zone_by_id, BiomeId, CampDef, HeightStamp, HubDef, LakeDef,
    ZoneBand, CAMPS, JAIL_TERRAIN_EDITS, LAKE_BLEND_RADIUS_MULT, SOWFIELD_FLAT_FALLOFF,
    SOWFIELD_FLAT_HEIGHT, SOWFIELD_FLAT_X_MAX, SOWFIELD_FLAT_X_MIN, SOWFIELD_FLAT_Z_MAX,
    SOWFIELD_FLAT_Z_MIN, WATER_LEVEL, WORLD_MAX_X, WORLD_MAX_Z, WORLD_MIN_Z, WORLD_SEED,
    WORLD_SIZE, ZONES, ZONE_EASTBROOK, ZONE_MIREFEN, ZONE_THORNPEAK,
};
pub use zone1::{MobSpot, NpcSpot, ZoneLayout, EASTBROOK};
pub use zone2::{EASTFEN, MIREFEN};
pub use zone3::THORNPEAK;

/// Known zone id strings (aliases + canonical upstream ids).
pub const KNOWN_ZONE_IDS: &[&str] = &[
    "eastbrook",
    "eastbrook_vale",
    "eastfen",
    "mirefen",
    "mirefen_marsh",
    "thornpeak",
    "thornpeak_heights",
    "fenbridge",
    "highwatch",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gear_item_has_rules() {
        for it in ITEMS.iter() {
            if it.equip_slot.is_none() {
                assert!(it.armor_class.is_none());
                assert!(it.weapon_style.is_none());
                continue;
            }
            match it.kind {
                ItemKind::Weapon => {
                    assert!(it.weapon_style.is_some(), "{}", it.id);
                    assert!(it.armor_class.is_none(), "{}", it.id);
                }
                ItemKind::Armor => {
                    let style = it.weapon_style;
                    if matches!(it.equip_slot, Some(ItemEquipSlot::OffHand)) {
                        assert_eq!(style, Some(WeaponStyle::Shield), "{}", it.id);
                        assert!(it.armor_class.is_none(), "{}", it.id);
                    } else if matches!(
                        it.equip_slot,
                        Some(ItemEquipSlot::Neck | ItemEquipSlot::Finger | ItemEquipSlot::Trinket)
                    ) {
                        assert!(style.is_none(), "{}", it.id);
                        assert!(it.armor_class.is_none(), "{}", it.id);
                    } else {
                        assert!(it.armor_class.is_some(), "{}", it.id);
                        assert!(style.is_none(), "{}", it.id);
                    }
                }
                _ => panic!("{} is equippable but not weapon/armor", it.id),
            }
        }
    }

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
    fn gear_has_max_durability() {
        assert_eq!(item("worn_sword").unwrap().max_durability, 40);
        assert_eq!(item("recruit_tunic").unwrap().max_durability, 30);
        assert_eq!(item("baked_bread").unwrap().max_durability, 0);
        assert_eq!(item("boar_tusk").unwrap().max_durability, 0);
    }

    #[test]
    fn every_class_has_multi_ability_kit() {
        assert_eq!(CLASSES.len(), 9);
        for class in CLASSES {
            assert_eq!(
                class.kit.len(),
                5,
                "{} kit needs 5 abilities, got {}",
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
                    panic!(
                        "{} kit refs missing ability {}",
                        class.name, entry.ability_id
                    )
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
                    ability(e.ability_id)
                        .map(|a| a.min_level > 1)
                        .unwrap_or(false)
                }),
                "{} needs at least one level-gated ability",
                class.name
            );
        }
    }

    #[test]
    fn restored_class_depth_kit_slots() {
        assert_eq!(
            class_ability_for_slot(PlayerClass::Hunter, 3)
                .expect("hunter 3")
                .id,
            "multi_shot"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Priest, 3)
                .expect("priest 3")
                .id,
            "shadow_word_pain"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Mage, 3)
                .expect("mage 3")
                .id,
            "counterspell"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Rogue, 5)
                .expect("rogue 5")
                .id,
            "sprint"
        );
    }

    #[test]
    fn kit_slot_lookup_matches_keys() {
        for class in CLASSES {
            let primary = class_ability_for_slot(class.id, 1).expect("slot 1");
            assert_eq!(primary.id, class.primary_ability);
            assert!(class_ability_for_slot(class.id, 2).is_some());
            assert!(class_ability_for_slot(class.id, 3).is_some());
            assert!(class_ability_for_slot(class.id, 4).is_some());
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
    fn every_quest_giver_and_turn_in_npc_is_marked_quest_giver() {
        for q in QUESTS.iter() {
            let giver = npc(q.giver_npc).unwrap_or_else(|| panic!("missing giver {}", q.giver_npc));
            assert!(
                giver.is_quest_giver(),
                "quest {} giver {} must have is_quest_giver",
                q.id,
                q.giver_npc
            );
            let turn_in = q.turn_in_npc.unwrap_or(q.giver_npc);
            if turn_in != q.giver_npc {
                let npc_def = npc(turn_in).unwrap_or_else(|| panic!("missing turn-in {turn_in}"));
                assert!(
                    npc_def.is_quest_giver(),
                    "quest {} turn-in {} must have is_quest_giver",
                    q.id,
                    turn_in
                );
            }
        }
    }

    #[test]
    fn every_quest_requires_exists_and_is_acyclic() {
        for q in QUESTS.iter() {
            let Some(req) = q.requires else {
                continue;
            };
            assert!(
                QUESTS.iter().any(|o| o.id == req),
                "quest {} requires missing {req}",
                q.id
            );
            let mut seen = vec![q.id];
            let mut cursor = q.requires;
            while let Some(id) = cursor {
                assert!(
                    !seen.contains(&id),
                    "quest {} has a requires cycle at {id}",
                    q.id
                );
                seen.push(id);
                cursor = quest(id).and_then(|d| d.requires);
            }
        }
    }

    #[test]
    fn eastbrook_quest_chain_is_report_wolves_tusks() {
        assert_eq!(quest("report_to_alden").unwrap().requires, None);
        assert_eq!(
            quest("wolves_at_the_gate").unwrap().requires,
            Some("report_to_alden")
        );
        assert_eq!(
            quest("boar_tusks").unwrap().requires,
            Some("wolves_at_the_gate")
        );
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
                    QuestObjective::Explore { x, z, radius, .. } => {
                        assert!(
                            *radius > 0.0,
                            "quest {} explore radius must be positive",
                            q.id
                        );
                        assert!(
                            x.abs() <= WORLD_MAX_X && *z >= WORLD_MIN_Z && *z <= WORLD_MAX_Z,
                            "quest {} explore point ({x},{z}) out of world",
                            q.id
                        );
                    }
                    QuestObjective::Escort {
                        npc_id,
                        dest_x,
                        dest_z,
                        radius,
                        ..
                    } => {
                        assert!(
                            NPCS.iter().any(|n| n.id == *npc_id),
                            "quest {} missing escort npc {npc_id}",
                            q.id
                        );
                        assert!(
                            *radius > 0.0,
                            "quest {} escort radius must be positive",
                            q.id
                        );
                        assert!(
                            dest_x.abs() <= WORLD_MAX_X
                                && *dest_z >= WORLD_MIN_Z
                                && *dest_z <= WORLD_MAX_Z,
                            "quest {} escort dest ({dest_x},{dest_z}) out of world",
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
            for item_id in q.reward.choices {
                assert!(
                    ITEMS.iter().any(|i| i.id == *item_id),
                    "quest {} missing choice item {item_id}",
                    q.id
                );
            }
        }
    }

    #[test]
    fn quest_depth_demo_rows_exist() {
        assert_eq!(quest("scout_north_road").unwrap().repeat, QuestRepeat::Once);
        assert_eq!(quest("wolf_patrol").unwrap().repeat, QuestRepeat::Daily);
        assert!(matches!(
            quest("courier_to_the_gate").unwrap().objectives[0],
            QuestObjective::Escort {
                npc_id: "eastbrook_courier",
                ..
            }
        ));
        assert_eq!(
            quest("arms_of_the_watch").unwrap().reward.choices,
            &["travelers_ration", "spring_water", "baked_bread"]
        );
        assert!(npc("trader_wilkes").unwrap().is_quest_giver());
        assert!(npc("eastbrook_courier").is_some());
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
    fn dungeon_bosses_have_mob_templates() {
        for d in DUNGEONS {
            assert!(
                mob(d.boss_id).is_some(),
                "boss {} missing MobTemplate",
                d.boss_id
            );
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
    fn npc_services_roster_locked() {
        let alden = npc("captain_alden").unwrap();
        assert!(alden.is_quest_giver());
        assert!(alden.is_class_trainer());
        assert!(!alden.is_vendor());

        let smith = npc("smith_brann").unwrap();
        assert!(smith.is_vendor());
        assert!(smith.can_repair());
        assert!(smith.trains_profession("mining"));
        assert!(smith.trains_profession("blacksmithing"));
        assert!(!smith.trains_profession("herbalism"));
        assert!(smith
            .vendor_stock
            .iter()
            .any(|o| o.item_id == "copper_shortsword"));

        let wren = npc("herbalist_wren").unwrap();
        assert!(wren.trains_profession("herbalism"));
        assert!(wren.trains_profession("alchemy"));
        assert!(wren.trains_profession("enchanting"));
        assert!(!wren.is_vendor());

        assert!(npc("innkeeper_mara").unwrap().is_innkeeper());
        assert!(npc("apothecary_vex").unwrap().trains_profession("alchemy"));
        assert!(npc("quartermaster_bren").unwrap().can_repair());
        assert!(npc("stable_master_ross").unwrap().is_riding_trainer());
        assert!(npc("auctioneer_lise").unwrap().is_auctioneer());
        assert!(npc("banker_holme").unwrap().is_banker());
        assert!(npc("mailbox_post").unwrap().is_mailbox());
        assert_eq!(
            npc("trader_wilkes").unwrap().faction,
            Some("eastbrook_watch")
        );
        assert!(npc("trader_wilkes")
            .unwrap()
            .vendor_stock
            .iter()
            .any(|o| o.item_id == "watch_signet" && o.min_standing == crate::Standing::Friendly));
    }

    #[test]
    fn stable_master_ross_roster() {
        let ross = npc("stable_master_ross").expect("ross");
        assert!(ross.is_riding_trainer());
        assert!(ross.is_vendor());
        assert!(!ross.is_profession_trainer());
        assert!(ross.trains.is_empty());
        let stock: Vec<_> = ross.vendor_stock.iter().map(|o| o.item_id).collect();
        assert!(stock.contains(&"brown_pony"));
        assert!(stock.contains(&"swift_bay_steed"));
        assert!(stock.contains(&"tawny_gryphon"));
        assert!(EASTBROOK.npcs.iter().any(|s| {
            s.npc_id == "stable_master_ross" && (s.x - 4.0).abs() < 1e-6 && (s.z - 9.0).abs() < 1e-6
        }));
    }

    #[test]
    fn riding_trainers_stock_mounts() {
        for n in NPCS.iter() {
            if n.services.contains(&NpcService::RidingTrainer) {
                assert!(n.is_vendor(), "{} riding trainer must vendor", n.id);
                assert!(!n.vendor_stock.is_empty(), "{} empty stock", n.id);
                for offer in n.vendor_stock {
                    let it = item(offer.item_id).unwrap();
                    assert_eq!(it.kind, ItemKind::Mount, "{} stocks non-mount", n.id);
                }
            }
        }
    }

    #[test]
    fn auctioneer_lise_is_eastbrook_auction_only() {
        let lise = npc("auctioneer_lise").expect("auctioneer_lise");
        assert!(lise.is_auctioneer());
        assert!(!lise.is_vendor());
        assert!(!lise.can_repair());
        assert!(lise.vendor_stock.is_empty());
        assert!(lise.trains.is_empty());
        assert!(EASTBROOK.npcs.iter().any(|s| s.npc_id == "auctioneer_lise"
            && (s.x - 4.0).abs() < f32::EPSILON
            && (s.z - 6.0).abs() < f32::EPSILON));
    }

    #[test]
    fn banker_and_mailbox_are_eastbrook_only_services() {
        let holme = npc("banker_holme").expect("banker_holme");
        assert!(holme.is_banker());
        assert!(!holme.is_auctioneer());
        assert!(!holme.is_vendor());
        assert_eq!(holme.greeting, "Your coin is safer with me.");
        assert!(EASTBROOK.npcs.iter().any(|s| s.npc_id == "banker_holme"
            && (s.x - 6.0).abs() < f32::EPSILON
            && (s.z - 6.0).abs() < f32::EPSILON));

        let post = npc("mailbox_post").expect("mailbox_post");
        assert!(post.is_mailbox());
        assert!(!post.is_banker());
        assert_eq!(post.greeting, "Leave it. We'll see it through.");
        assert!(EASTBROOK.npcs.iter().any(|s| s.npc_id == "mailbox_post"
            && (s.x - 0.0).abs() < f32::EPSILON
            && (s.z - 8.0).abs() < f32::EPSILON));
    }

    #[test]
    fn catalog_bind_rules() {
        use crate::ItemBind;
        assert_eq!(item("worn_sword").unwrap().bind, ItemBind::OnEquip);
        assert_eq!(item("recruit_tunic").unwrap().bind, ItemBind::OnEquip);
        assert_eq!(item("boar_tusk").unwrap().bind, ItemBind::OnPickup);
        assert_eq!(item("silverleaf").unwrap().bind, ItemBind::None);
        assert_eq!(item("travelers_ration").unwrap().bind, ItemBind::None);
    }

    #[test]
    fn factions_and_reputation_tables_are_consistent() {
        use crate::{faction, FACTIONS};

        assert_eq!(FACTIONS.len(), 4);
        for n in NPCS.iter() {
            if let Some(id) = n.faction {
                assert!(faction(id).is_some(), "{} unknown faction {id}", n.id);
            }
            for offer in n.vendor_stock {
                assert!(
                    ITEMS.iter().any(|i| i.id == offer.item_id),
                    "vendor {} missing gated item {}",
                    n.id,
                    offer.item_id
                );
            }
        }
        for q in QUESTS.iter() {
            if let Some(rep) = q.reward.reputation {
                assert!(
                    faction(rep.faction_id).is_some(),
                    "quest {} unknown faction {}",
                    q.id,
                    rep.faction_id
                );
                assert!(rep.amount > 0, "quest {} reputation must be positive", q.id);
            }
        }
        for m in MOBS.iter() {
            if let Some(rep) = m.kill_reputation {
                assert!(
                    faction(rep.faction_id).is_some(),
                    "mob {} unknown faction {}",
                    m.id,
                    rep.faction_id
                );
                assert!(
                    rep.amount > 0,
                    "mob {} kill reputation must be positive",
                    m.id
                );
            }
        }
        assert_eq!(
            quest("report_to_alden")
                .unwrap()
                .reward
                .reputation
                .unwrap()
                .amount,
            150
        );
        assert_eq!(
            quest("wolves_at_the_gate")
                .unwrap()
                .reward
                .reputation
                .unwrap()
                .faction_id,
            "eastbrook_watch"
        );
        assert_eq!(
            item("watch_signet").unwrap().equip_slot,
            Some(ItemEquipSlot::Finger)
        );
    }

    #[test]
    fn profession_trainers_reference_known_professions() {
        use crate::NpcService;

        for n in NPCS.iter() {
            if n.services.contains(&NpcService::ProfessionTrainer) {
                assert!(!n.trains.is_empty(), "{} trains nothing", n.id);
            }
            for id in n.trains {
                assert!(profession(id).is_some(), "{} trains unknown {id}", n.id);
            }
        }
    }

    #[test]
    fn vendors_have_stock_and_buyable_prices() {
        use crate::NpcService;

        for n in NPCS.iter() {
            if !n.services.contains(&NpcService::Vendor) {
                continue;
            }
            assert!(
                !n.vendor_stock.is_empty(),
                "vendor {} has empty stock",
                n.id
            );
            for o in n.vendor_stock {
                let def = item(o.item_id).expect(o.item_id);
                assert!(
                    def.vendor_buy > 0,
                    "{} sells {} at vendor_buy 0",
                    n.id,
                    o.item_id
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
        assert!(dungeon("mirefen_barrow").is_some());
        assert!(dungeon("missing_dungeon").is_none());
    }

    #[test]
    fn mob_abilities_exist() {
        assert!(ability("wolf_bite").is_some());
        assert!(ability("warden_smash").is_some());
        assert!(ability("terror_slam").is_some());
        assert_eq!(mob("scarred_wolf").unwrap().ability_id, Some("wolf_bite"));
        assert_eq!(
            mob("crypt_warden").unwrap().ability_id,
            Some("warden_smash")
        );
        assert_eq!(mob("mire_terror").unwrap().ability_id, Some("terror_slam"));
    }

    #[test]
    fn every_ability_declares_an_effect() {
        assert_eq!(ABILITIES.len(), 56);
        for def in ABILITIES {
            let _ = def.effect;
            if let Some(aura_id) = def.aura {
                assert!(
                    aura(aura_id).is_some(),
                    "ability {} aura {} missing from AURAS",
                    def.id,
                    aura_id
                );
            }
            if matches!(def.effect, AbilityEffect::ApplyAura) {
                assert!(
                    def.aura.is_some(),
                    "ApplyAura ability {} needs an aura id",
                    def.id
                );
            }
        }
        assert!(matches!(
            ability("cleave").unwrap().effect,
            AbilityEffect::AoeDamage { .. }
        ));
        assert!(matches!(
            ability("flash_heal").unwrap().effect,
            AbilityEffect::Heal { .. }
        ));
        assert!(matches!(
            ability("taunt").unwrap().effect,
            AbilityEffect::Taunt { .. }
        ));
        assert!(matches!(
            ability("earth_shock").unwrap().effect,
            AbilityEffect::Interrupt
        ));
        assert!(matches!(
            ability("execute").unwrap().effect,
            AbilityEffect::Execute { .. }
        ));
        assert!(matches!(
            ability("holy_shock").unwrap().effect,
            AbilityEffect::HealOrHarm { .. }
        ));
        assert!(matches!(
            ability("power_word_shield").unwrap().effect,
            AbilityEffect::Absorb { .. }
        ));
        assert!(matches!(
            ability("charge").unwrap().effect,
            AbilityEffect::Charge { .. }
        ));
        assert_eq!(
            class_def(PlayerClass::Hunter).resource_type,
            ResourceType::Mana
        );
    }

    #[test]
    fn class_identity_slot5_signatures() {
        assert_eq!(
            class_ability_for_slot(PlayerClass::Priest, 5)
                .expect("priest 5")
                .id,
            "power_word_shield"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Warrior, 5)
                .expect("warrior 5")
                .id,
            "charge"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Mage, 5)
                .expect("mage 5")
                .id,
            "blink"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Hunter, 5)
                .expect("hunter 5")
                .id,
            "aspect_of_the_hawk"
        );
        assert!(ability("rend").is_some());
        assert!(ability("shadow_word_pain").is_some());
        assert!(ability("counterspell").is_some());
        assert!(ability("multi_shot").is_some());
    }

    #[test]
    fn class_forms_kit_signatures() {
        assert_eq!(
            class_ability_for_slot(PlayerClass::Shaman, 5)
                .expect("shaman 5")
                .id,
            "lightning_shield"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Warlock, 4)
                .expect("warlock 4")
                .id,
            "life_tap"
        );
        assert_eq!(
            class_ability_for_slot(PlayerClass::Warlock, 5)
                .expect("warlock 5")
                .id,
            "fear"
        );
        assert!(ability("immolate").is_some());
        assert!(ability("flame_shock").is_some());
        assert_eq!(
            ability("crusader_strike").unwrap().aura,
            Some("seal_righteousness")
        );
    }

    #[test]
    fn every_class_kit_has_distinct_effects() {
        use std::mem::discriminant;
        for class in CLASSES {
            let mut unique = Vec::new();
            for entry in class.kit {
                let def = ability(entry.ability_id).expect(entry.ability_id);
                let kind = discriminant(&def.effect);
                if !unique.contains(&kind) {
                    unique.push(kind);
                }
            }
            assert!(
                unique.len() >= 2,
                "{} kit needs ≥2 distinct AbilityEffect kinds, got {}",
                class.name,
                unique.len()
            );
        }
        for class_id in ["paladin", "shaman", "druid", "priest"] {
            let class = CLASSES
                .iter()
                .find(|c| c.id.as_str() == class_id)
                .expect(class_id);
            assert!(
                class.kit.iter().any(|e| {
                    matches!(
                        ability(e.ability_id).map(|a| a.effect),
                        Some(AbilityEffect::Heal { .. } | AbilityEffect::HealOrHarm { .. })
                    )
                }),
                "{class_id} needs a heal"
            );
        }
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
    fn thornpeak_layout_has_resolvable_unique_content() {
        assert!(
            THORNPEAK.npcs.len() >= 3,
            "thornpeak needs ≥3 NPC spots, got {}",
            THORNPEAK.npcs.len()
        );
        assert!(
            THORNPEAK.mobs.len() >= 6,
            "thornpeak needs ≥6 mob spots, got {}",
            THORNPEAK.mobs.len()
        );

        let mob_templates = THORNPEAK
            .mobs
            .iter()
            .map(|spot| spot.mob_id)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            mob_templates.len() >= 3,
            "thornpeak needs ≥3 distinct mob templates, got {}",
            mob_templates.len()
        );

        for npc_id in ["commander_elara", "pathfinder_toren", "quartermaster_bren"] {
            assert!(npc(npc_id).is_some(), "missing Thornpeak NPC {npc_id}");
            assert!(
                THORNPEAK.npcs.iter().any(|spot| spot.npc_id == npc_id),
                "Thornpeak layout missing NPC {npc_id}"
            );
        }
        for mob_id in ["ridge_stalker", "cragback_boar", "gale_harpy"] {
            assert!(mob(mob_id).is_some(), "missing Thornpeak mob {mob_id}");
            assert!(
                THORNPEAK.mobs.iter().any(|spot| spot.mob_id == mob_id),
                "Thornpeak layout missing mob {mob_id}"
            );
        }
    }

    #[test]
    fn thornpeak_has_dedicated_quests_and_graveyard() {
        let quest_ids = [
            "stalkers_on_the_ridge",
            "tusks_for_highwatch",
            "harpies_over_highwatch",
        ];
        let quests = quest_ids
            .iter()
            .map(|id| quest(id).unwrap_or_else(|| panic!("missing Thornpeak quest {id}")))
            .collect::<Vec<_>>();

        assert!(quests.iter().all(|quest| {
            quest.objectives.iter().all(|objective| {
                matches!(
                    objective,
                    QuestObjective::Kill { .. } | QuestObjective::Collect { .. }
                )
            })
        }));
        assert!(
            quests.iter().any(|quest| {
                quest
                    .objectives
                    .iter()
                    .any(|objective| matches!(objective, QuestObjective::Kill { .. }))
            }),
            "Thornpeak needs a kill quest"
        );
        assert!(
            quests.iter().any(|quest| {
                quest
                    .objectives
                    .iter()
                    .any(|objective| matches!(objective, QuestObjective::Collect { .. }))
            }),
            "Thornpeak needs a collect quest"
        );
        assert!(graveyard("thornpeak_graveyard").is_some());
        assert!(graveyard_for_zone("thornpeak").is_some());
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
        assert!(profession("mining").is_some());
        assert!(profession("blacksmithing").is_some());
        assert!(profession("skinning").is_some());
        assert!(profession("leatherworking").is_some());
        assert!(profession("tailoring").is_some());
        assert!(profession("jewelcrafting").is_some());
        assert!(profession("enchanting").is_some());
        assert!(profession("engineering").is_some());
        assert_eq!(PROFESSIONS.len(), 10);
        assert_eq!(profession("mining").unwrap().max_skill, 100);
        assert_eq!(profession("blacksmithing").unwrap().max_skill, 125);
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
            assert!(item(node.tool_item_id).is_some());
        }
        assert!(gather_node("eastbrook_meadow_silverleaf").is_some());
        assert!(gather_nodes_for_zone("eastbrook").count() >= 3);
        let mining_nodes = GATHER_NODES
            .iter()
            .filter(|n| n.profession_id == "mining")
            .count();
        assert!(
            mining_nodes >= 3,
            "expected ≥3 mining nodes, got {mining_nodes}"
        );
        assert!(gather_node("eastbrook_south_copper").is_some());
        assert!(gather_nodes_for_zone("eastfen").any(|n| n.profession_id == "mining"));
        let sword = ITEMS
            .iter()
            .find(|i| i.id == "copper_shortsword")
            .expect("copper shortsword");
        assert_eq!(sword.equip_slot, Some(ItemEquipSlot::MainHand));
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
        assert!(recipe("smelt_copper_bar").is_some());
        assert!(recipe("copper_shortsword").is_some());
        assert!(recipes_for_profession("blacksmithing").count() >= 2);
        assert!(recipe("copper_shortsword").unwrap().station == Some("forge"));
        assert!(recipe("tigerseye_band").is_some());
        assert!(recipe("copper_grenade").is_some());
        assert!(recipe("linen_trousers").is_some());
        assert!(recipe("light_leather_jerkin").is_some());
    }

    #[test]
    fn recipe_input_value_exceeds_output_vendor_sell() {
        for recipe in RECIPES {
            let input: u32 = recipe
                .reagents
                .iter()
                .map(|r| reagent_unit_value(item(r.item_id).expect(r.item_id)) * r.count)
                .sum();
            let output = item(recipe.product_item_id)
                .expect(recipe.product_item_id)
                .vendor_sell
                * recipe.product_count;
            assert!(
                input > output,
                "{} input {input} must exceed output {output}",
                recipe.id
            );
            if let Some(station_id) = recipe.station {
                assert!(
                    station(station_id).is_some(),
                    "missing station {station_id}"
                );
            }
        }
    }

    #[test]
    fn vendors_never_stock_gathered_mats() {
        for n in NPCS.iter() {
            for offer in n.vendor_stock {
                assert!(
                    !item_is_gathered(offer.item_id),
                    "{} stocks gathered {}",
                    n.id,
                    offer.item_id
                );
            }
        }
    }

    #[test]
    fn gathered_buy_is_four_times_sell() {
        for def in ITEMS.iter().filter(|d| item_is_gathered(d.id)) {
            assert_eq!(
                def.vendor_buy,
                def.vendor_sell * 4,
                "{} buy/sell ratio",
                def.id
            );
        }
    }

    #[test]
    fn overworld_wolves_respawn_in_thirty_seconds() {
        let w = mob("young_wolf").expect("young_wolf");
        assert!((w.respawn_seconds - 30.0).abs() < f32::EPSILON);
        assert!(w.ability_id.is_none());
    }

    #[test]
    fn world_boss_respawns_in_five_minutes() {
        let t = mob("mire_terror").expect("mire_terror");
        assert!((t.respawn_seconds - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn eastbrook_wolf_run_is_a_pack() {
        let wolves: u32 = EASTBROOK
            .mobs
            .iter()
            .filter(|s| s.mob_id == "young_wolf")
            .map(|s| s.count)
            .sum();
        assert!(wolves >= 5, "wolf run count={wolves}");
        for spot in EASTBROOK.mobs {
            assert!(spot.count >= 1);
            assert!(spot.radius > 0.0);
        }
    }

    #[test]
    fn stations_and_enchants_are_registered() {
        assert_eq!(STATIONS.len(), 6);
        assert!(in_station_range(0.0, 0.0, "forge"));
        assert!(!in_station_range(50.0, 50.0, "forge"));
        assert_eq!(PROFESSION_ENCHANTS.len(), 3);
        assert!(profession_enchant("weapon_minor_might").is_some());
        assert!(enchant("weapon_minor_might").is_some());
        assert_eq!(
            disenchant_yield("copper_shortsword")[0].item_id,
            "arcane_dust"
        );
    }

    #[test]
    fn riding_ranks_locked() {
        assert_eq!(RIDING_RANKS.len(), 3);
        let a = riding_rank("apprentice").expect("apprentice");
        assert_eq!(a.rank, 1);
        assert_eq!(a.level_req, 2);
        assert_eq!(a.copper, 10);
        assert!((a.ground_speed_mult - 1.6).abs() < 1e-6);
        assert_eq!(riding_rank_by_n(3).unwrap().id, "expert");
        assert!(riding_rank_by_n(0).is_none());
    }

    #[test]
    fn mount_table_matches_items() {
        assert_eq!(MOUNTS.len(), 3);
        for def in MOUNTS.iter() {
            let it = item(def.item_id).unwrap_or_else(|| panic!("missing {}", def.item_id));
            assert_eq!(it.kind, ItemKind::Mount, "{}", def.id);
            assert_eq!(it.stack_size, 1);
            assert_eq!(it.max_durability, 0);
            assert!(it.equip_slot.is_none());
            assert_eq!(mount_by_item(def.item_id).map(|m| m.id), Some(def.id));
        }
        let pony = mount("brown_pony").unwrap();
        assert_eq!(pony.riding_rank, 1);
        assert!(matches!(pony.kind, MountKind::Ground));
        assert!((pony.speed_mult - 1.6).abs() < 1e-6);
        let gryphon = mount("tawny_gryphon").unwrap();
        assert!(matches!(gryphon.kind, MountKind::Flying));
        assert_eq!(gryphon.riding_rank, 3);
        assert_eq!(item("brown_pony").unwrap().vendor_buy, 25);
        assert_eq!(item("swift_bay_steed").unwrap().vendor_buy, 150);
        assert_eq!(item("tawny_gryphon").unwrap().vendor_buy, 300);
    }
}
