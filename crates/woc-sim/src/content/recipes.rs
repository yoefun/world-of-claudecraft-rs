use crate::item::ItemId;
use crate::professions::types::{ProfessionId, RecipeDef, RecipeId, Reagent, StationType};

pub const RECIPES: &[RecipeDef] = &[
    RecipeDef {
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
    },
    RecipeDef {
        id: RecipeId::CopperShortsword,
        profession: ProfessionId::Forging,
        result: ItemId::CopperShortsword,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::CopperBar,
                count: 3,
            },
            Reagent {
                item: ItemId::SmithingFlux,
                count: 2,
            },
        ],
        skill_req: 0,
        item_level_budget: 10,
        station: Some(StationType::Forge),
    },
    RecipeDef {
        id: RecipeId::CopperChainVest,
        profession: ProfessionId::Forging,
        result: ItemId::CopperChainVest,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::CopperBar,
                count: 5,
            },
            Reagent {
                item: ItemId::SmithingFlux,
                count: 3,
            },
        ],
        skill_req: 0,
        item_level_budget: 10,
        station: Some(StationType::Forge),
    },
    RecipeDef {
        id: RecipeId::CopperPick,
        profession: ProfessionId::Forging,
        result: ItemId::CopperPick,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::CopperBar,
                count: 3,
            },
            Reagent {
                item: ItemId::CoarseStone,
                count: 2,
            },
        ],
        skill_req: 0,
        item_level_budget: 8,
        station: Some(StationType::Forge),
    },
];

pub fn recipe_by_id(id: RecipeId) -> Option<&'static RecipeDef> {
    RECIPES.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::Gold;
    use crate::inventory::{Inventory, ItemStack};
    use crate::professions::crafting::{complete_craft, evaluate_craft_admission};
    use crate::professions::skill::ProfessionSkills;
    use crate::professions::types::{DenyReason, ProfessionId, RecipeId, Vec2};
    use crate::rng::ScriptedRng;

    fn forge_pos() -> Vec2 {
        Vec2 { x: 0.0, z: 0.0 }
    }

    fn tannery_pos() -> Vec2 {
        Vec2 { x: 80.0, z: 40.0 }
    }

    fn inv_with_shortsword_reagents() -> Inventory {
        let mut inv = Inventory::with_capacity(8);
        inv.try_add(ItemStack {
            item: ItemId::CopperBar,
            count: 3,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::SmithingFlux,
            count: 2,
        })
        .unwrap();
        inv
    }

    #[test]
    fn shortsword_requires_forge() {
        let inv = inv_with_shortsword_reagents();
        let gold = Gold { copper: 100 };
        let err = evaluate_craft_admission(
            RecipeId::CopperShortsword,
            1,
            tannery_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::StationRequired);
    }

    #[test]
    fn shortsword_crafts_at_forge() {
        let mut inv = inv_with_shortsword_reagents();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::CopperShortsword,
            1,
            forge_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::CopperShortsword,
            1,
            forge_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(grant.gold_spent, 20);
        assert_eq!(inv.count(ItemId::CopperShortsword), 1);
        assert_eq!(inv.count(ItemId::CopperBar), 0);
        assert_eq!(inv.count(ItemId::SmithingFlux), 0);
        assert_eq!(gold.copper, 80);
        assert_eq!(skills.get(ProfessionId::Forging), 2);
    }
}
