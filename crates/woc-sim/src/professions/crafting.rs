use crate::content::recipes::recipe_by_id;
use crate::gold::Gold;
use crate::inventory::{Inventory, ItemStack};
use crate::item::ItemId;
use crate::rng::Rng;
use super::masterwork::masterwork_proc_chance;
use super::skill::ProfessionSkills;
use super::stations::in_station_range;
use super::types::{
    CRAFT_BATCH_MAX, CRAFT_GOLD_SINK_COPPER_PER_BUDGET, DenyReason, RecipeDef, RecipeId, Reagent,
    Vec2,
};

pub fn base_of(item: ItemId) -> ItemId {
    match item {
        ItemId::FineCopperOre => ItemId::CopperOre,
        ItemId::FineSilverleaf => ItemId::Silverleaf,
        ItemId::FineEarthroot => ItemId::Earthroot,
        ItemId::FineLightLeather => ItemId::LightLeather,
        other => other,
    }
}

fn fine_substitute_for(base: ItemId) -> Option<ItemId> {
    match base {
        ItemId::CopperOre => Some(ItemId::FineCopperOre),
        ItemId::Silverleaf => Some(ItemId::FineSilverleaf),
        ItemId::Earthroot => Some(ItemId::FineEarthroot),
        ItemId::LightLeather => Some(ItemId::FineLightLeather),
        _ => None,
    }
}

fn available_count(inv: &Inventory, item: ItemId) -> u16 {
    let exact = inv.count(item);
    if item != base_of(item) {
        return exact;
    }
    exact + fine_substitute_for(item)
        .map(|fine| inv.count(fine))
        .unwrap_or(0)
}

fn has_reagents(inv: &Inventory, reagents: &[Reagent], crafts: u16) -> bool {
    reagents.iter().all(|r| available_count(inv, r.item) >= r.count.saturating_mul(crafts))
}

fn remove_reagent(inv: &mut Inventory, item: ItemId, count: u16) -> bool {
    let mut remaining = count;
    let from_exact = inv.count(item).min(remaining);
    if from_exact > 0 && inv.try_remove(item, from_exact).is_err() {
        return false;
    }
    remaining = remaining.saturating_sub(from_exact);
    if remaining == 0 {
        return true;
    }
    if item == base_of(item) {
        if let Some(fine) = fine_substitute_for(item) {
            return inv.try_remove(fine, remaining).is_ok();
        }
    }
    false
}

fn remove_reagents(inv: &mut Inventory, reagents: &[Reagent]) -> bool {
    reagents
        .iter()
        .all(|r| remove_reagent(inv, r.item, r.count))
}

fn craft_fee(recipe: &RecipeDef) -> u32 {
    u32::from(recipe.item_level_budget) * CRAFT_GOLD_SINK_COPPER_PER_BUDGET
}

fn total_craft_fee(recipe: &RecipeDef, count: u16) -> u32 {
    craft_fee(recipe) * u32::from(count)
}

fn can_fit_batch(inv: &Inventory, recipe: &RecipeDef, count: u16) -> bool {
    let mut trial = inv.clone();
    for _ in 0..count {
        let stack = ItemStack {
            item: recipe.result,
            count: recipe.result_count,
        };
        if trial.try_add(stack).is_err() {
            return false;
        }
    }
    true
}

pub fn evaluate_craft_admission(
    recipe_id: RecipeId,
    count: u16,
    pos: Vec2,
    inv: &Inventory,
    gold: &Gold,
    busy: bool,
) -> Result<&'static RecipeDef, DenyReason> {
    if count == 0 || count > CRAFT_BATCH_MAX {
        return Err(DenyReason::InvalidCount);
    }
    let recipe = recipe_by_id(recipe_id).ok_or(DenyReason::UnknownRecipe)?;
    if busy {
        return Err(DenyReason::Busy);
    }
    if let Some(station) = recipe.station {
        if !in_station_range(pos, station) {
            return Err(DenyReason::StationRequired);
        }
    }
    if !has_reagents(inv, recipe.reagents, count) {
        return Err(DenyReason::MissingReagents);
    }
    if gold.copper < total_craft_fee(recipe, count) {
        return Err(DenyReason::InsufficientGold);
    }
    if !can_fit_batch(inv, recipe, count) {
        return Err(DenyReason::InventoryFull);
    }
    Ok(recipe)
}

#[derive(Debug)]
pub struct CraftGrant {
    pub items_crafted: u16,
    pub skill_gained: u16,
    pub gold_spent: u32,
}

pub fn complete_craft(
    recipe_id: RecipeId,
    count: u16,
    pos: Vec2,
    inv: &mut Inventory,
    gold: &mut Gold,
    skills: &mut ProfessionSkills,
    busy: bool,
    last_masterwork: &mut Option<RecipeId>,
    rng: &mut impl Rng,
) -> Result<CraftGrant, DenyReason> {
    let recipe = evaluate_craft_admission(recipe_id, count, pos, inv, gold, busy)?;
    let fee = craft_fee(recipe);
    let mut items_crafted = 0u16;
    let mut skill_gained = 0u16;
    let mut gold_spent = 0u32;

    for _ in 0..count {
        if !has_reagents(inv, recipe.reagents, 1) {
            break;
        }
        if gold.copper < fee {
            break;
        }
        let stack = ItemStack {
            item: recipe.result,
            count: recipe.result_count,
        };
        let mut trial = inv.clone();
        if trial.try_add(stack).is_err() {
            break;
        }

        if !remove_reagents(inv, recipe.reagents) {
            break;
        }
        inv.try_add(ItemStack {
            item: recipe.result,
            count: recipe.result_count,
        })
        .map_err(|_| DenyReason::InventoryFull)?;
        gold.try_spend(fee);
        gold_spent += fee;

        let chance = masterwork_proc_chance(skills.get(recipe.profession), recipe.skill_req);
        if rng.chance(chance) {
            *last_masterwork = Some(recipe.id);
        }
        skill_gained += skills.gain(recipe.profession, recipe.skill_req);
        items_crafted += 1;
    }

    if items_crafted == 0 {
        return Err(DenyReason::MissingReagents);
    }

    Ok(CraftGrant {
        items_crafted,
        skill_gained,
        gold_spent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::Gold;
    use crate::inventory::Inventory;
    use crate::professions::skill::ProfessionSkills;
    use crate::professions::types::ProfessionId;
    use crate::rng::ScriptedRng;

    fn field_pos() -> Vec2 {
        Vec2 { x: 50.0, z: 50.0 }
    }

    #[test]
    fn missing_ore_does_not_charge_gold() {
        let inv = Inventory::with_capacity(4);
        let gold = Gold { copper: 100 };
        let err = evaluate_craft_admission(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::MissingReagents);

        let mut inv = inv;
        let mut gold = gold;
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = complete_craft(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::MissingReagents);
        assert_eq!(gold.copper, 100);
        assert_eq!(inv.count(ItemId::CopperBar), 0);
    }

    #[test]
    fn fine_ore_substitutes_downward() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::FineCopperOre,
            count: 2,
        })
        .unwrap();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(inv.count(ItemId::CopperBar), 1);
        assert_eq!(inv.count(ItemId::FineCopperOre), 0);
        assert_eq!(gold.copper, 98);
        assert_eq!(skills.get(ProfessionId::Forging), 2);
    }

    #[test]
    fn smelt_is_field_craftable() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 2,
        })
        .unwrap();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(inv.count(ItemId::CopperBar), 1);
        assert_eq!(inv.count(ItemId::CopperOre), 0);
        assert_eq!(gold.copper, 98);
    }

    #[test]
    fn base_of_maps_fine_materials_to_base() {
        assert_eq!(base_of(ItemId::FineCopperOre), ItemId::CopperOre);
        assert_eq!(base_of(ItemId::CopperOre), ItemId::CopperOre);
    }

    #[test]
    fn exact_ore_consumed_before_fine() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 1,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::FineCopperOre,
            count: 1,
        })
        .unwrap();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        complete_craft(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(inv.count(ItemId::CopperOre), 0);
        assert_eq!(inv.count(ItemId::FineCopperOre), 0);
    }

    #[test]
    fn denied_craft_draws_zero() {
        let inv = Inventory::with_capacity(4);
        let gold = Gold { copper: 100 };
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = evaluate_craft_admission(
            RecipeId::SmeltCopper,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::MissingReagents);
        let _ = &mut rng;
    }
}
