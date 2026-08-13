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
    RecipeDef {
        id: RecipeId::CureLightLeather,
        profession: ProfessionId::Leatherworking,
        result: ItemId::CuredLightLeather,
        result_count: 1,
        reagents: &[Reagent {
            item: ItemId::LightLeather,
            count: 1,
        }],
        skill_req: 0,
        item_level_budget: 1,
        station: None,
    },
    RecipeDef {
        id: RecipeId::LightLeatherJerkin,
        profession: ProfessionId::Leatherworking,
        result: ItemId::LightLeatherJerkin,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::CuredLightLeather,
                count: 4,
            },
            Reagent {
                item: ItemId::SpoolOfThread,
                count: 2,
            },
        ],
        skill_req: 0,
        item_level_budget: 9,
        station: Some(StationType::Tannery),
    },
    RecipeDef {
        id: RecipeId::LightLeatherBelt,
        profession: ProfessionId::Leatherworking,
        result: ItemId::LightLeatherBelt,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::CuredLightLeather,
                count: 2,
            },
            Reagent {
                item: ItemId::SpoolOfThread,
                count: 1,
            },
        ],
        skill_req: 0,
        item_level_budget: 6,
        station: Some(StationType::Tannery),
    },
    RecipeDef {
        id: RecipeId::BoltOfLinen,
        profession: ProfessionId::Tailoring,
        result: ItemId::BoltOfLinen,
        result_count: 1,
        reagents: &[Reagent {
            item: ItemId::LinenCloth,
            count: 2,
        }],
        skill_req: 0,
        item_level_budget: 1,
        station: None,
    },
    RecipeDef {
        id: RecipeId::LinenTrousers,
        profession: ProfessionId::Tailoring,
        result: ItemId::LinenTrousers,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::BoltOfLinen,
                count: 3,
            },
            Reagent {
                item: ItemId::SpoolOfThread,
                count: 2,
            },
        ],
        skill_req: 0,
        item_level_budget: 8,
        station: Some(StationType::Loom),
    },
    RecipeDef {
        id: RecipeId::LinenVestments,
        profession: ProfessionId::Tailoring,
        result: ItemId::LinenVestments,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::BoltOfLinen,
                count: 4,
            },
            Reagent {
                item: ItemId::SpoolOfThread,
                count: 3,
            },
        ],
        skill_req: 0,
        item_level_budget: 9,
        station: Some(StationType::Loom),
    },
    RecipeDef {
        id: RecipeId::ProspectCopper,
        profession: ProfessionId::Jewelcrafting,
        result: ItemId::Tigerseye,
        result_count: 1,
        reagents: &[Reagent {
            item: ItemId::CopperOre,
            count: 5,
        }],
        skill_req: 0,
        item_level_budget: 2,
        station: None,
    },
    RecipeDef {
        id: RecipeId::CopperSetting,
        profession: ProfessionId::Jewelcrafting,
        result: ItemId::CopperSetting,
        result_count: 1,
        reagents: &[Reagent {
            item: ItemId::CopperBar,
            count: 1,
        }],
        skill_req: 0,
        item_level_budget: 1,
        station: None,
    },
    RecipeDef {
        id: RecipeId::TigerseyeBand,
        profession: ProfessionId::Jewelcrafting,
        result: ItemId::TigerseyeBand,
        result_count: 1,
        reagents: &[
            Reagent {
                item: ItemId::Tigerseye,
                count: 1,
            },
            Reagent {
                item: ItemId::CopperSetting,
                count: 1,
            },
        ],
        skill_req: 0,
        item_level_budget: 8,
        station: Some(StationType::JewelersBench),
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

    fn field_pos() -> Vec2 {
        Vec2 { x: 50.0, z: 50.0 }
    }

    fn forge_pos() -> Vec2 {
        Vec2 { x: 0.0, z: 0.0 }
    }

    fn tannery_pos() -> Vec2 {
        Vec2 { x: 80.0, z: 40.0 }
    }

    fn loom_pos() -> Vec2 {
        Vec2 { x: 20.0, z: -10.0 }
    }

    fn inv_with_light_leather() -> Inventory {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::LightLeather,
            count: 1,
        })
        .unwrap();
        inv
    }

    fn inv_with_jerkin_reagents() -> Inventory {
        let mut inv = Inventory::with_capacity(8);
        inv.try_add(ItemStack {
            item: ItemId::CuredLightLeather,
            count: 4,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::SpoolOfThread,
            count: 2,
        })
        .unwrap();
        inv
    }

    fn inv_with_linen_cloth() -> Inventory {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::LinenCloth,
            count: 2,
        })
        .unwrap();
        inv
    }

    fn inv_with_trousers_reagents() -> Inventory {
        let mut inv = Inventory::with_capacity(8);
        inv.try_add(ItemStack {
            item: ItemId::BoltOfLinen,
            count: 3,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::SpoolOfThread,
            count: 2,
        })
        .unwrap();
        inv
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

    fn jewelers_bench_pos() -> Vec2 {
        Vec2 { x: 15.0, z: 5.0 }
    }

    fn inv_with_copper_ore() -> Inventory {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 5,
        })
        .unwrap();
        inv
    }

    fn inv_with_tigerseye_band_reagents() -> Inventory {
        let mut inv = Inventory::with_capacity(8);
        inv.try_add(ItemStack {
            item: ItemId::Tigerseye,
            count: 1,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::CopperSetting,
            count: 1,
        })
        .unwrap();
        inv
    }

    #[test]
    fn prospect_copper_is_field_craftable_and_deterministic() {
        let mut inv = inv_with_copper_ore();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::ProspectCopper,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::ProspectCopper,
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
        assert_eq!(inv.count(ItemId::Tigerseye), 1);
        assert_eq!(inv.count(ItemId::CopperOre), 0);
        assert_eq!(skills.get(ProfessionId::Jewelcrafting), 2);
    }

    #[test]
    fn tigerseye_band_requires_jewelers_bench() {
        let inv = inv_with_tigerseye_band_reagents();
        let gold = Gold { copper: 100 };
        let err = evaluate_craft_admission(
            RecipeId::TigerseyeBand,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::StationRequired);

        let mut inv = inv_with_tigerseye_band_reagents();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::TigerseyeBand,
            1,
            jewelers_bench_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::TigerseyeBand,
            1,
            jewelers_bench_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(inv.count(ItemId::TigerseyeBand), 1);
        assert_eq!(inv.count(ItemId::Tigerseye), 0);
        assert_eq!(inv.count(ItemId::CopperSetting), 0);
        assert_eq!(skills.get(ProfessionId::Jewelcrafting), 2);
    }

    #[test]
    fn bolt_of_linen_is_field_craftable() {
        let mut inv = inv_with_linen_cloth();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::BoltOfLinen,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::BoltOfLinen,
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
        assert_eq!(inv.count(ItemId::BoltOfLinen), 1);
        assert_eq!(inv.count(ItemId::LinenCloth), 0);
        assert_eq!(gold.copper, 98);
        assert_eq!(skills.get(ProfessionId::Tailoring), 2);
    }

    #[test]
    fn trousers_require_loom() {
        let inv = inv_with_trousers_reagents();
        let gold = Gold { copper: 100 };
        let err = evaluate_craft_admission(
            RecipeId::LinenTrousers,
            1,
            forge_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::StationRequired);

        let mut inv = inv_with_trousers_reagents();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::LinenTrousers,
            1,
            loom_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::LinenTrousers,
            1,
            loom_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(grant.gold_spent, 16);
        assert_eq!(inv.count(ItemId::LinenTrousers), 1);
        assert_eq!(inv.count(ItemId::BoltOfLinen), 0);
        assert_eq!(inv.count(ItemId::SpoolOfThread), 0);
        assert_eq!(gold.copper, 84);
        assert_eq!(skills.get(ProfessionId::Tailoring), 2);
    }

    #[test]
    fn curing_hide_is_field_craftable() {
        let mut inv = inv_with_light_leather();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::CureLightLeather,
            1,
            field_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::CureLightLeather,
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
        assert_eq!(inv.count(ItemId::CuredLightLeather), 1);
        assert_eq!(inv.count(ItemId::LightLeather), 0);
        assert_eq!(gold.copper, 98);
        assert_eq!(skills.get(ProfessionId::Leatherworking), 2);
    }

    #[test]
    fn jerkin_requires_tannery() {
        let inv = inv_with_jerkin_reagents();
        let gold = Gold { copper: 100 };
        let err = evaluate_craft_admission(
            RecipeId::LightLeatherJerkin,
            1,
            forge_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::StationRequired);

        let mut inv = inv_with_jerkin_reagents();
        let mut gold = Gold { copper: 100 };
        let mut skills = ProfessionSkills::default();
        let mut last_masterwork = None;
        let mut rng = ScriptedRng::from_seq(&[99]);

        evaluate_craft_admission(
            RecipeId::LightLeatherJerkin,
            1,
            tannery_pos(),
            &inv,
            &gold,
            false,
        )
        .unwrap();

        let grant = complete_craft(
            RecipeId::LightLeatherJerkin,
            1,
            tannery_pos(),
            &mut inv,
            &mut gold,
            &mut skills,
            false,
            &mut last_masterwork,
            &mut rng,
        )
        .unwrap();

        assert_eq!(grant.items_crafted, 1);
        assert_eq!(grant.gold_spent, 18);
        assert_eq!(inv.count(ItemId::LightLeatherJerkin), 1);
        assert_eq!(inv.count(ItemId::CuredLightLeather), 0);
        assert_eq!(inv.count(ItemId::SpoolOfThread), 0);
        assert_eq!(gold.copper, 82);
        assert_eq!(skills.get(ProfessionId::Leatherworking), 2);
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
