use super::skill::ProfessionSkills;
use super::tools::{best_tool_tier, can_gather_tier, profession_for_node};
use super::types::{
    DenyReason, GatherNodeDef, NodeId, NodeKind, ProfessionId, Vec2, HARVEST_RANGE,
};
use crate::content::nodes::{herb_is_earthroot, node_by_id};
use crate::inventory::{Inventory, ItemStack};
use crate::item::ItemId;
use crate::rng::Rng;
use crate::TICK_HZ;

pub fn evaluate_gather(
    pos: Vec2,
    inv: &Inventory,
    node: &GatherNodeDef,
    ready_tick: u64,
    now: u64,
    busy: bool,
) -> Result<ProfessionId, DenyReason> {
    if busy {
        return Err(DenyReason::Busy);
    }
    if pos.distance(node.pos) > HARVEST_RANGE {
        return Err(DenyReason::OutOfRange);
    }
    if now < ready_tick {
        return Err(DenyReason::NodeNotReady);
    }
    let profession = profession_for_node(node.kind);
    let tool_tier = best_tool_tier(inv, profession).ok_or(DenyReason::MissingTool)?;
    if !can_gather_tier(tool_tier, node.tier) {
        return Err(DenyReason::ToolTierTooLow);
    }
    Ok(profession)
}

fn harvest_items(node: &GatherNodeDef) -> (ItemId, ItemId) {
    match node.kind {
        NodeKind::Ore => (ItemId::CopperOre, ItemId::FineCopperOre),
        NodeKind::Herb if herb_is_earthroot(node.id) => (ItemId::Earthroot, ItemId::FineEarthroot),
        NodeKind::Herb => (ItemId::Silverleaf, ItemId::FineSilverleaf),
    }
}

pub struct HarvestGrant {
    pub stacks: Vec<ItemStack>,
    pub skill_gained: u16,
    pub profession: ProfessionId,
    pub next_ready_tick: u64,
}

pub fn complete_gather(
    pos: Vec2,
    inv: &mut Inventory,
    skills: &mut ProfessionSkills,
    node: &GatherNodeDef,
    ready_tick: u64,
    now: u64,
    rng: &mut impl Rng,
) -> Result<HarvestGrant, DenyReason> {
    let profession = evaluate_gather(pos, inv, node, ready_tick, now, false)?;
    let tool_tier = best_tool_tier(inv, profession).expect("tool re-checked");
    let rare = rng.chance(15);
    let double = rng.chance(20);
    let (base, fine) = harvest_items(node);
    let use_fine = rare || tool_tier > node.tier;
    let item = if use_fine { fine } else { base };
    let count = if rare {
        5
    } else if double {
        2
    } else {
        1
    };
    let mut stacks = vec![ItemStack { item, count }];
    if node.kind == NodeKind::Ore && double && !rare {
        stacks.push(ItemStack {
            item: ItemId::CoarseStone,
            count: 1,
        });
    }
    let mut trial = inv.clone();
    for stack in &stacks {
        trial
            .try_add(*stack)
            .map_err(|_| DenyReason::InventoryFull)?;
    }
    *inv = trial;
    let skill_gained = skills.gain(profession, node.skill_req);
    Ok(HarvestGrant {
        stacks,
        skill_gained,
        profession,
        next_ready_tick: now + u64::from(node.respawn_seconds) * u64::from(TICK_HZ),
    })
}

pub fn start_gather_node(
    pos: Vec2,
    inv: &Inventory,
    node_id: NodeId,
    ready_tick: u64,
    now: u64,
    busy: bool,
) -> Result<&'static GatherNodeDef, DenyReason> {
    let node = node_by_id(node_id).ok_or(DenyReason::UnknownNode)?;
    evaluate_gather(pos, inv, node, ready_tick, now, busy)?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::nodes::node_by_id;
    use crate::inventory::Inventory;
    use crate::professions::skill::ProfessionSkills;
    use crate::professions::types::NodeId;
    use crate::rng::ScriptedRng;

    fn node1() -> &'static GatherNodeDef {
        node_by_id(NodeId(1)).unwrap()
    }

    #[test]
    fn bare_hands_cannot_mine() {
        let inv = Inventory::with_capacity(4);
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = evaluate_gather(node1().pos, &inv, node1(), 0, 0, false).unwrap_err();
        assert_eq!(err, DenyReason::MissingTool);
        let _ = &mut rng;
    }

    #[test]
    fn successful_ore_harvest_draws_twice() {
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::CopperPick,
            count: 1,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();
        let mut rng = ScriptedRng::from_seq(&[99, 99]);
        let grant =
            complete_gather(node1().pos, &mut inv, &mut skills, node1(), 0, 0, &mut rng).unwrap();
        assert_eq!(grant.stacks[0].item, ItemId::CopperOre);
        assert_eq!(grant.stacks[0].count, 1);
        assert_eq!(skills.get(ProfessionId::Mining), 2);
        assert_eq!(grant.next_ready_tick, 60 * 20);
    }

    #[test]
    fn double_ore_harvest_failure_leaves_inventory_unchanged() {
        let mut inv = Inventory::with_capacity(2);
        inv.try_add(ItemStack {
            item: ItemId::CopperPick,
            count: 1,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::CopperOre,
            count: 1,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();
        let mut rng = ScriptedRng::from_seq(&[99, 0]);

        let err = match complete_gather(node1().pos, &mut inv, &mut skills, node1(), 0, 0, &mut rng)
        {
            Ok(_) => panic!("double ore harvest unexpectedly fit in a full bag"),
            Err(err) => err,
        };

        assert_eq!(err, DenyReason::InventoryFull);
        assert_eq!(inv.count(ItemId::CopperOre), 1);
        assert_eq!(inv.count(ItemId::CopperPick), 1);
        assert_eq!(skills.get(ProfessionId::Mining), 0);
    }

    #[test]
    fn denied_harvest_draws_zero() {
        let inv = Inventory::with_capacity(4);
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = evaluate_gather(node1().pos, &inv, node1(), 0, 0, false).unwrap_err();
        assert_eq!(err, DenyReason::MissingTool);
        let _ = &mut rng;
    }
}
