use super::types::{NodeKind, ProfessionId};
use crate::inventory::Inventory;
use crate::item::ItemId;

pub fn tool_item_for(profession: ProfessionId) -> Option<ItemId> {
    match profession {
        ProfessionId::Mining => Some(ItemId::CopperPick),
        ProfessionId::Herbalism => Some(ItemId::CopperSickle),
        ProfessionId::Skinning => Some(ItemId::SkinningKnife),
        _ => None,
    }
}

pub fn profession_for_node(kind: NodeKind) -> ProfessionId {
    match kind {
        NodeKind::Ore => ProfessionId::Mining,
        NodeKind::Herb => ProfessionId::Herbalism,
    }
}

/// v1 tools are all tier 1. Presence in bags is the gate.
pub fn best_tool_tier(inv: &Inventory, profession: ProfessionId) -> Option<u8> {
    let item = tool_item_for(profession)?;
    if inv.has(item) {
        Some(1)
    } else {
        None
    }
}

pub fn can_gather_tier(tool_tier: u8, node_tier: u8) -> bool {
    tool_tier >= node_tier
}
