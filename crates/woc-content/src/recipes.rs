//! Crafting recipe definitions.

#[derive(Debug, Clone, Copy)]
pub struct RecipeReagent {
    pub item_id: &'static str,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct RecipeDef {
    pub id: &'static str,
    pub name: &'static str,
    pub profession_id: &'static str,
    pub skill_req: u32,
    pub reagents: &'static [RecipeReagent],
    pub product_item_id: &'static str,
    pub product_count: u32,
}

/// Alchemy recipes consuming Eastbrook herbs into usable salves / tonics.
pub static RECIPES: &[RecipeDef] = &[
    RecipeDef {
        id: "minor_healing_salve",
        name: "Minor Healing Salve",
        profession_id: "alchemy",
        skill_req: 1,
        reagents: &[
            RecipeReagent {
                item_id: "silverleaf",
                count: 2,
            },
            RecipeReagent {
                item_id: "peacebloom",
                count: 1,
            },
        ],
        product_item_id: "minor_healing_salve",
        product_count: 1,
    },
    RecipeDef {
        id: "briar_tonic",
        name: "Briar Tonic",
        profession_id: "alchemy",
        skill_req: 1,
        reagents: &[
            RecipeReagent {
                item_id: "briarroot",
                count: 1,
            },
            RecipeReagent {
                item_id: "silverleaf",
                count: 1,
            },
        ],
        product_item_id: "briar_tonic",
        product_count: 1,
    },
];

pub fn recipe(id: &str) -> Option<&'static RecipeDef> {
    RECIPES.iter().find(|r| r.id == id)
}

/// Recipes taught by a given profession id.
pub fn recipes_for_profession(
    profession_id: &str,
) -> impl Iterator<Item = &'static RecipeDef> + '_ {
    RECIPES
        .iter()
        .filter(move |r| r.profession_id == profession_id)
}
