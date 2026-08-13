use crate::inventory::{Inventory, ItemStack};
use crate::item::ItemId;
use crate::rng::Rng;
use super::skill::ProfessionSkills;
use super::tools::{best_tool_tier, can_gather_tier};
use super::types::{Corpse, DenyReason, HARVEST_RANGE, ProfessionId, Vec2};

const SKIN_SKILL_REQ: u16 = 0;

pub fn evaluate_skin(
    pos: Vec2,
    inv: &Inventory,
    corpse: &Corpse,
    busy: bool,
) -> Result<(), DenyReason> {
    if busy {
        return Err(DenyReason::Busy);
    }
    if pos.distance(corpse.pos) > HARVEST_RANGE {
        return Err(DenyReason::OutOfRange);
    }
    if !corpse.has_hide {
        return Err(DenyReason::NothingToSkin);
    }
    if corpse.skinned {
        return Err(DenyReason::AlreadySkinned);
    }
    let tool_tier = best_tool_tier(inv, ProfessionId::Skinning).ok_or(DenyReason::MissingKnife)?;
    if !can_gather_tier(tool_tier, corpse.tier) {
        return Err(DenyReason::ToolTierTooLow);
    }
    Ok(())
}

fn can_accept_stacks(inv: &Inventory, stacks: &[ItemStack]) -> bool {
    let mut trial = inv.clone();
    for stack in stacks {
        if trial.try_add(*stack).is_err() {
            return false;
        }
    }
    true
}

fn can_accept_any_skin(inv: &Inventory) -> bool {
    let normal = [ItemStack {
        item: ItemId::LightLeather,
        count: 1,
    }];
    let double = [ItemStack {
        item: ItemId::LightLeather,
        count: 2,
    }];
    let rare = [ItemStack {
        item: ItemId::FineLightLeather,
        count: 5,
    }];
    can_accept_stacks(inv, &normal)
        || can_accept_stacks(inv, &double)
        || can_accept_stacks(inv, &rare)
}

#[derive(Debug)]
pub struct SkinGrant {
    pub stacks: Vec<ItemStack>,
    pub skill_gained: u16,
}

pub fn complete_skin(
    pos: Vec2,
    inv: &mut Inventory,
    skills: &mut ProfessionSkills,
    corpse: &mut Corpse,
    rng: &mut impl Rng,
) -> Result<SkinGrant, DenyReason> {
    evaluate_skin(pos, inv, corpse, false)?;
    if !can_accept_any_skin(inv) {
        return Err(DenyReason::InventoryFull);
    }
    let tool_tier = best_tool_tier(inv, ProfessionId::Skinning).expect("knife re-checked");
    let rare = rng.chance(15);
    let double = rng.chance(20);
    let use_fine = rare || tool_tier > corpse.tier;
    let item = if use_fine {
        ItemId::FineLightLeather
    } else {
        ItemId::LightLeather
    };
    let count = if rare { 5 } else if double { 2 } else { 1 };
    let stacks = vec![ItemStack { item, count }];
    for stack in &stacks {
        inv.try_add(*stack).map_err(|_| DenyReason::InventoryFull)?;
    }
    corpse.skinned = true;
    let skill_gained = skills.gain(ProfessionId::Skinning, SKIN_SKILL_REQ);
    Ok(SkinGrant {
        stacks,
        skill_gained,
    })
}

pub fn start_skin(
    pos: Vec2,
    inv: &Inventory,
    corpse: &Corpse,
    busy: bool,
) -> Result<(), DenyReason> {
    evaluate_skin(pos, inv, corpse, busy)?;
    if !can_accept_any_skin(inv) {
        return Err(DenyReason::InventoryFull);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Inventory;
    use crate::professions::skill::ProfessionSkills;
    use crate::professions::types::CorpseId;
    use crate::rng::ScriptedRng;

    fn hide_corpse() -> Corpse {
        Corpse {
            id: CorpseId(1),
            pos: Vec2 { x: 0.0, z: 0.0 },
            has_hide: true,
            skinned: false,
            tier: 1,
        }
    }

    #[test]
    fn untagged_corpse_is_not_claimed() {
        let mut corpse = Corpse {
            id: CorpseId(1),
            pos: Vec2 { x: 0.0, z: 0.0 },
            has_hide: false,
            skinned: false,
            tier: 1,
        };
        let inv = Inventory::with_capacity(4);
        let err = start_skin(corpse.pos, &inv, &corpse, false).unwrap_err();
        assert_eq!(err, DenyReason::NothingToSkin);
        assert!(!corpse.skinned);
        let mut skills = ProfessionSkills::default();
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = complete_skin(
            corpse.pos,
            &mut Inventory::with_capacity(4),
            &mut skills,
            &mut corpse,
            &mut rng,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::NothingToSkin);
        assert!(!corpse.skinned);
    }

    #[test]
    fn hide_corpse_yields_light_leather_once() {
        let mut corpse = hide_corpse();
        let mut inv = Inventory::with_capacity(4);
        inv.try_add(ItemStack {
            item: ItemId::SkinningKnife,
            count: 1,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();
        let mut rng = ScriptedRng::from_seq(&[99, 99]);
        start_skin(corpse.pos, &inv, &corpse, false).unwrap();
        let grant = complete_skin(corpse.pos, &mut inv, &mut skills, &mut corpse, &mut rng).unwrap();
        assert_eq!(grant.stacks[0].item, ItemId::LightLeather);
        assert_eq!(grant.stacks[0].count, 1);
        assert_eq!(skills.get(ProfessionId::Skinning), 2);
        assert!(corpse.skinned);
        let err = complete_skin(corpse.pos, &mut inv, &mut skills, &mut corpse, &mut rng).unwrap_err();
        assert_eq!(err, DenyReason::AlreadySkinned);
    }

    #[test]
    fn missing_knife_is_not_missing_tool() {
        let corpse = hide_corpse();
        let inv = Inventory::with_capacity(4);
        let mut rng = ScriptedRng::from_seq(&[]);
        let err = start_skin(corpse.pos, &inv, &corpse, false).unwrap_err();
        assert_eq!(err, DenyReason::MissingKnife);
        let _ = &mut rng;
    }
}
