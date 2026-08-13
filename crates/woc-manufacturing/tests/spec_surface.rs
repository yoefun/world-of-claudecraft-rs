//! Grep the design spec against the shipped crate surface.

use woc_manufacturing::content::items::{item_def, ITEM_DEFS};
use woc_manufacturing::content::recipes::RECIPES;
use woc_manufacturing::content::stations::STATIONS;
use woc_manufacturing::item::{reagent_unit_value, ItemId};
use woc_manufacturing::professions::skill::ProfessionSkills;
use woc_manufacturing::professions::types::{DenyReason, ProfessionId, RecipeId, StationType};

#[test]
fn spec_lists_ten_profession_ids() {
    assert_eq!(ProfessionId::ALL.len(), 10);
    let names: Vec<_> = ProfessionId::ALL
        .iter()
        .map(|p| format!("{p:?}"))
        .collect();
    assert_eq!(
        names,
        [
            "Mining",
            "Herbalism",
            "Skinning",
            "Forging",
            "Leatherworking",
            "Tailoring",
            "Jewelcrafting",
            "Enchanting",
            "Engineering",
            "Alchemy",
        ]
    );
}

#[test]
fn profession_skills_has_ten_independent_counters() {
    let mut skills = ProfessionSkills::default();
    for id in ProfessionId::ALL {
        assert_eq!(skills.get(id), 0);
        skills.gain(id, 0);
        assert_eq!(skills.get(id), 2);
    }
    for id in ProfessionId::ALL {
        assert_eq!(skills.get(id), 2);
    }
}

#[test]
fn spec_recipe_ids_are_shipped() {
    let expected = [
        RecipeId::SmeltCopper,
        RecipeId::CopperShortsword,
        RecipeId::CopperChainVest,
        RecipeId::CopperPick,
        RecipeId::CureLightLeather,
        RecipeId::LightLeatherJerkin,
        RecipeId::LightLeatherBelt,
        RecipeId::BoltOfLinen,
        RecipeId::LinenTrousers,
        RecipeId::LinenVestments,
        RecipeId::ProspectCopper,
        RecipeId::CopperSetting,
        RecipeId::TigerseyeBand,
        RecipeId::MinorHealingPotion,
        RecipeId::ElixirOfMinorStrength,
        RecipeId::RoughBlastingPowder,
        RecipeId::CopperBolt,
        RecipeId::CopperGrenade,
    ];
    assert_eq!(expected.len(), RECIPES.len());
    for (recipe, id) in RECIPES.iter().zip(expected.iter()) {
        assert_eq!(recipe.id, *id);
    }
}

#[test]
fn spec_deny_reasons_exist() {
    let reasons = [
        DenyReason::OutOfRange,
        DenyReason::NodeNotReady,
        DenyReason::MissingTool,
        DenyReason::ToolTierTooLow,
        DenyReason::InventoryFull,
        DenyReason::UnknownNode,
        DenyReason::Busy,
        DenyReason::CorpseGone,
        DenyReason::NothingToSkin,
        DenyReason::AlreadySkinned,
        DenyReason::MissingKnife,
        DenyReason::UnknownRecipe,
        DenyReason::MissingReagents,
        DenyReason::InsufficientGold,
        DenyReason::StationRequired,
        DenyReason::InvalidCount,
        DenyReason::UnknownEnchant,
        DenyReason::WrongSlot,
        DenyReason::AlreadyEnchanted,
        DenyReason::SameEnchant,
        DenyReason::NotInstanced,
    ];
    assert_eq!(reasons.len(), 21);
}

#[test]
fn spec_stations_include_loom_and_jewelers_bench() {
    let kinds: Vec<_> = STATIONS.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&StationType::Loom));
    assert!(kinds.contains(&StationType::JewelersBench));
}

#[test]
fn copper_grenade_sell_value_is_eight() {
    assert_eq!(item_def(ItemId::CopperGrenade).sell_value, 8);
}

#[test]
fn spec_economy_formula_holds_for_every_recipe() {
    for recipe in RECIPES {
        let input: u32 = recipe
            .reagents
            .iter()
            .map(|r| reagent_unit_value(item_def(r.item)) * u32::from(r.count))
            .sum();
        let output = item_def(recipe.result).sell_value * u32::from(recipe.result_count);
        assert!(
            input > output,
            "{:?}: input {input} must exceed output {output}",
            recipe.id
        );
    }
}

#[test]
fn every_item_def_is_addressable() {
    assert_eq!(ITEM_DEFS.len(), 36);
}
