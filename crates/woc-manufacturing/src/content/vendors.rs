use crate::item::ItemId;

/// Processed goods only. Gathered / skinned materials MUST NOT appear here.
pub const VENDOR_ITEMS: &[ItemId] = &[
    ItemId::SmithingFlux,
    ItemId::SpoolOfThread,
    ItemId::EmptyVial,
    ItemId::LinenCloth,
    ItemId::CopperPick,
    ItemId::CopperSickle,
    ItemId::SkinningKnife,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::items::ITEM_DEFS;

    #[test]
    fn vendor_never_stocks_gathered() {
        let gathered: std::collections::HashSet<ItemId> = ITEM_DEFS
            .iter()
            .filter(|d| d.gathered)
            .map(|d| d.id)
            .collect();
        for &item in VENDOR_ITEMS {
            assert!(
                !gathered.contains(&item),
                "vendor must not stock gathered item {:?}",
                item
            );
        }
    }
}
