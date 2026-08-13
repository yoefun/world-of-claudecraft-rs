use crate::item::ItemId;
use crate::professions::types::{ProfessionId, RecipeDef, RecipeId, Reagent};

pub const RECIPES: &[RecipeDef] = &[RecipeDef {
    id: RecipeId::SmeltCopper,
    profession: ProfessionId::Forging,
    result: ItemId::CopperBar,
    result_count: 1,
    reagents: &[Reagent {
        item: ItemId::CopperOre,
        count: 2,
    }],
    skill_req: 0,
    item_level_budget: 1,
    station: None,
}];

pub fn recipe_by_id(id: RecipeId) -> Option<&'static RecipeDef> {
    RECIPES.iter().find(|r| r.id == id)
}
