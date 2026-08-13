#[cfg(test)]
mod tests {
    use crate::content::enchants::ENCHANT_DEFS;
    use crate::content::items::{item_def, ITEM_DEFS};
    use crate::content::recipes::RECIPES;
    use crate::content::vendors::VENDOR_ITEMS;
    use crate::item::{reagent_unit_value, ItemId, Quality};
    use crate::content::enchants::disenchant_yield;

    #[test]
    fn every_recipe_costs_more_than_it_vendors() {
        for recipe in RECIPES {
            let input: u32 = recipe
                .reagents
                .iter()
                .map(|r| reagent_unit_value(item_def(r.item)) * u32::from(r.count))
                .sum();
            let output = item_def(recipe.result).sell_value * u32::from(recipe.result_count);
            assert!(
                input > output,
                "{:?} input {input} must exceed output {output}",
                recipe.id
            );
        }
    }

    #[test]
    fn every_enchant_costs_more_than_dust_vendor_loop() {
        let common_dust_yield: u32 = disenchant_yield(Quality::Common)
            .iter()
            .map(|r| item_def(r.item).sell_value * u32::from(r.count))
            .sum();

        for enchant in ENCHANT_DEFS {
            let input: u32 = enchant
                .reagents
                .iter()
                .map(|r| {
                    let def = item_def(r.item);
                    assert!(
                        !def.gathered,
                        "{:?} reagent {:?} must not be gathered",
                        enchant.id,
                        r.item
                    );
                    reagent_unit_value(def) * u32::from(r.count)
                })
                .sum();
            assert!(input > 0, "{:?} must consume valued reagents", enchant.id);
            assert!(
                input > common_dust_yield,
                "{:?} input {input} must exceed common disenchant dust value {common_dust_yield}",
                enchant.id
            );
        }
    }

    #[test]
    fn vendors_never_stock_gathered_or_skinned_mats() {
        for id in VENDOR_ITEMS {
            assert!(!item_def(*id).gathered, "{id:?} must not be vendored");
        }

        let gathered: std::collections::HashSet<ItemId> = ITEM_DEFS
            .iter()
            .filter(|d| d.gathered)
            .map(|d| d.id)
            .collect();
        for &item in VENDOR_ITEMS {
            assert!(
                !gathered.contains(&item),
                "vendor must not stock gathered/skinned item {:?}",
                item
            );
        }
    }
}
