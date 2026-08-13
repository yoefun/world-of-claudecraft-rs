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
    /// Known recipes do not use this as an admission gate.
    pub skill_req: u32,
    pub reagents: &'static [RecipeReagent],
    pub product_item_id: &'static str,
    pub product_count: u32,
    pub station: Option<&'static str>,
    pub item_level_budget: u32,
}

pub static RECIPES: &[RecipeDef] = &[
    RecipeDef {
        id: "minor_healing_salve",
        name: "Minor Healing Salve",
        profession_id: "alchemy",
        skill_req: 0,
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
        station: None,
        item_level_budget: 0,
    },
    RecipeDef {
        id: "briar_tonic",
        name: "Briar Tonic",
        profession_id: "alchemy",
        skill_req: 0,
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
        station: None,
        item_level_budget: 0,
    },
    RecipeDef {
        id: "smelt_copper_bar",
        name: "Smelt Copper Bar",
        profession_id: "blacksmithing",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "copper_ore",
            count: 2,
        }],
        product_item_id: "copper_bar",
        product_count: 1,
        station: None,
        item_level_budget: 1,
    },
    RecipeDef {
        id: "copper_shortsword",
        name: "Copper Shortsword",
        profession_id: "blacksmithing",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "copper_bar",
                count: 3,
            },
            RecipeReagent {
                item_id: "smithing_flux",
                count: 2,
            },
        ],
        product_item_id: "copper_shortsword",
        product_count: 1,
        station: Some("forge"),
        item_level_budget: 10,
    },
    RecipeDef {
        id: "copper_chain_vest",
        name: "Copper Chain Vest",
        profession_id: "blacksmithing",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "copper_bar",
                count: 5,
            },
            RecipeReagent {
                item_id: "smithing_flux",
                count: 3,
            },
        ],
        product_item_id: "copper_chain_vest",
        product_count: 1,
        station: Some("forge"),
        item_level_budget: 10,
    },
    RecipeDef {
        id: "copper_pick",
        name: "Copper Pick",
        profession_id: "blacksmithing",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "copper_bar",
                count: 3,
            },
            RecipeReagent {
                item_id: "coarse_stone",
                count: 2,
            },
        ],
        product_item_id: "copper_pick",
        product_count: 1,
        station: Some("forge"),
        item_level_budget: 8,
    },
    RecipeDef {
        id: "cure_light_leather",
        name: "Cure Light Leather",
        profession_id: "leatherworking",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "light_leather",
            count: 1,
        }],
        product_item_id: "cured_light_leather",
        product_count: 1,
        station: None,
        item_level_budget: 1,
    },
    RecipeDef {
        id: "light_leather_jerkin",
        name: "Light Leather Jerkin",
        profession_id: "leatherworking",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "cured_light_leather",
                count: 4,
            },
            RecipeReagent {
                item_id: "spool_of_thread",
                count: 2,
            },
        ],
        product_item_id: "light_leather_jerkin",
        product_count: 1,
        station: Some("tannery"),
        item_level_budget: 9,
    },
    RecipeDef {
        id: "light_leather_belt",
        name: "Light Leather Belt",
        profession_id: "leatherworking",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "cured_light_leather",
                count: 2,
            },
            RecipeReagent {
                item_id: "spool_of_thread",
                count: 1,
            },
        ],
        product_item_id: "light_leather_belt",
        product_count: 1,
        station: Some("tannery"),
        item_level_budget: 6,
    },
    RecipeDef {
        id: "bolt_of_linen",
        name: "Bolt of Linen",
        profession_id: "tailoring",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "linen_cloth",
            count: 2,
        }],
        product_item_id: "bolt_of_linen",
        product_count: 1,
        station: None,
        item_level_budget: 1,
    },
    RecipeDef {
        id: "linen_trousers",
        name: "Linen Trousers",
        profession_id: "tailoring",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "bolt_of_linen",
                count: 3,
            },
            RecipeReagent {
                item_id: "spool_of_thread",
                count: 2,
            },
        ],
        product_item_id: "linen_trousers",
        product_count: 1,
        station: Some("loom"),
        item_level_budget: 8,
    },
    RecipeDef {
        id: "linen_vestments",
        name: "Linen Vestments",
        profession_id: "tailoring",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "bolt_of_linen",
                count: 4,
            },
            RecipeReagent {
                item_id: "spool_of_thread",
                count: 3,
            },
        ],
        product_item_id: "linen_vestments",
        product_count: 1,
        station: Some("loom"),
        item_level_budget: 9,
    },
    RecipeDef {
        id: "prospect_copper",
        name: "Prospect Copper",
        profession_id: "jewelcrafting",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "copper_ore",
            count: 5,
        }],
        product_item_id: "tigerseye",
        product_count: 1,
        station: None,
        item_level_budget: 2,
    },
    RecipeDef {
        id: "copper_setting",
        name: "Copper Setting",
        profession_id: "jewelcrafting",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "copper_bar",
            count: 1,
        }],
        product_item_id: "copper_setting",
        product_count: 1,
        station: None,
        item_level_budget: 1,
    },
    RecipeDef {
        id: "tigerseye_band",
        name: "Tigerseye Band",
        profession_id: "jewelcrafting",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "tigerseye",
                count: 1,
            },
            RecipeReagent {
                item_id: "copper_setting",
                count: 1,
            },
        ],
        product_item_id: "tigerseye_band",
        product_count: 1,
        station: Some("jewelers_bench"),
        item_level_budget: 8,
    },
    RecipeDef {
        id: "minor_healing_potion",
        name: "Minor Healing Potion",
        profession_id: "alchemy",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "silverleaf",
                count: 2,
            },
            RecipeReagent {
                item_id: "empty_vial",
                count: 1,
            },
        ],
        product_item_id: "minor_healing_potion",
        product_count: 1,
        station: Some("apothecary"),
        item_level_budget: 1,
    },
    RecipeDef {
        id: "elixir_of_minor_strength",
        name: "Elixir of Minor Strength",
        profession_id: "alchemy",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "earthroot",
                count: 2,
            },
            RecipeReagent {
                item_id: "empty_vial",
                count: 1,
            },
        ],
        product_item_id: "elixir_of_minor_strength",
        product_count: 1,
        station: Some("apothecary"),
        item_level_budget: 1,
    },
    RecipeDef {
        id: "rough_blasting_powder",
        name: "Rough Blasting Powder",
        profession_id: "engineering",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "coarse_stone",
            count: 2,
        }],
        product_item_id: "rough_blasting_powder",
        product_count: 1,
        station: None,
        item_level_budget: 1,
    },
    RecipeDef {
        id: "copper_bolt",
        name: "Copper Bolt",
        profession_id: "engineering",
        skill_req: 0,
        reagents: &[RecipeReagent {
            item_id: "copper_bar",
            count: 1,
        }],
        product_item_id: "copper_bolt",
        product_count: 2,
        station: Some("toolworks"),
        item_level_budget: 2,
    },
    RecipeDef {
        id: "copper_grenade",
        name: "Copper Grenade",
        profession_id: "engineering",
        skill_req: 0,
        reagents: &[
            RecipeReagent {
                item_id: "copper_bar",
                count: 1,
            },
            RecipeReagent {
                item_id: "rough_blasting_powder",
                count: 2,
            },
            RecipeReagent {
                item_id: "copper_bolt",
                count: 1,
            },
        ],
        product_item_id: "copper_grenade",
        product_count: 2,
        station: Some("toolworks"),
        item_level_budget: 6,
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

pub const CRAFT_GOLD_SINK_COPPER_PER_BUDGET: u32 = 2;
pub const CRAFT_BATCH_MAX: u32 = 50;

pub fn craft_fee(recipe: &RecipeDef) -> u32 {
    recipe.item_level_budget.saturating_mul(CRAFT_GOLD_SINK_COPPER_PER_BUDGET)
}
