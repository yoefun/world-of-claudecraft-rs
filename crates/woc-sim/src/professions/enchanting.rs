use crate::content::enchants::{disenchant_yield, enchant_by_id, EnchantId};
use crate::content::items::item_def;
use crate::inventory::{Inventory, ItemStack};
use crate::professions::types::{DenyReason, ProfessionId, Reagent};
use super::skill::ProfessionSkills;

const ENCHANT_SKILL_REQ: u16 = 0;

fn has_reagents(inv: &Inventory, reagents: &[Reagent]) -> bool {
    reagents
        .iter()
        .all(|r| inv.count(r.item) >= r.count)
}

fn remove_reagents(inv: &mut Inventory, reagents: &[Reagent]) -> bool {
    reagents
        .iter()
        .all(|r| inv.try_remove(r.item, r.count).is_ok())
}

fn can_accept_reagents(inv: &Inventory, reagents: &[Reagent]) -> bool {
    let mut trial = inv.clone();
    for reagent in reagents {
        if trial
            .try_add(ItemStack {
                item: reagent.item,
                count: reagent.count,
            })
            .is_err()
        {
            return false;
        }
    }
    true
}

pub fn evaluate_disenchant(
    instance_id: u64,
    inv: &Inventory,
    busy: bool,
) -> Result<&'static [Reagent], DenyReason> {
    if busy {
        return Err(DenyReason::Busy);
    }
    let instance = inv.instance(instance_id).ok_or(DenyReason::NotInstanced)?;
    let quality = item_def(instance.item).quality;
    let yield_reagents = disenchant_yield(quality);
    if !can_accept_reagents(inv, yield_reagents) {
        return Err(DenyReason::InventoryFull);
    }
    Ok(yield_reagents)
}

#[derive(Debug)]
pub struct DisenchantGrant {
    pub reagents: Vec<ItemStack>,
    pub skill_gained: u16,
}

pub fn complete_disenchant(
    instance_id: u64,
    inv: &mut Inventory,
    skills: &mut ProfessionSkills,
    busy: bool,
) -> Result<DisenchantGrant, DenyReason> {
    let yield_reagents = evaluate_disenchant(instance_id, inv, busy)?;
    let instance = inv
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or(DenyReason::NotInstanced)?;
    let item = instance.item;
    inv.instances.retain(|i| i.id != instance_id);

    let mut reagents = Vec::new();
    for reagent in yield_reagents {
        inv.try_add(ItemStack {
            item: reagent.item,
            count: reagent.count,
        })
        .map_err(|_| DenyReason::InventoryFull)?;
        reagents.push(ItemStack {
            item: reagent.item,
            count: reagent.count,
        });
    }
    let skill_gained = skills.gain(ProfessionId::Enchanting, ENCHANT_SKILL_REQ);
    let _ = item;
    Ok(DisenchantGrant {
        reagents,
        skill_gained,
    })
}

pub fn evaluate_apply_enchant(
    instance_id: u64,
    enchant_id: EnchantId,
    confirm_replace: bool,
    inv: &Inventory,
    busy: bool,
) -> Result<&'static crate::content::enchants::EnchantDef, DenyReason> {
    if busy {
        return Err(DenyReason::Busy);
    }
    let enchant = enchant_by_id(enchant_id);
    let instance = inv.instance(instance_id).ok_or(DenyReason::NotInstanced)?;
    let item_slot = item_def(instance.item).slot;
    if enchant.slot != item_slot {
        return Err(DenyReason::WrongSlot);
    }
    if let Some(current) = instance.enchant {
        if current == enchant_id {
            return Err(DenyReason::SameEnchant);
        }
        if !confirm_replace {
            return Err(DenyReason::AlreadyEnchanted);
        }
    }
    if !has_reagents(inv, enchant.reagents) {
        return Err(DenyReason::MissingReagents);
    }
    Ok(enchant)
}

#[derive(Debug)]
pub struct ApplyEnchantGrant {
    pub enchant: EnchantId,
    pub skill_gained: u16,
}

pub fn complete_apply_enchant(
    instance_id: u64,
    enchant_id: EnchantId,
    confirm_replace: bool,
    inv: &mut Inventory,
    skills: &mut ProfessionSkills,
    busy: bool,
) -> Result<ApplyEnchantGrant, DenyReason> {
    let enchant = evaluate_apply_enchant(instance_id, enchant_id, confirm_replace, inv, busy)?;
    if !remove_reagents(inv, enchant.reagents) {
        return Err(DenyReason::MissingReagents);
    }
    let instance = inv
        .instance_mut(instance_id)
        .ok_or(DenyReason::NotInstanced)?;
    instance.enchant = Some(enchant_id);
    let skill_gained = skills.gain(ProfessionId::Enchanting, ENCHANT_SKILL_REQ);
    Ok(ApplyEnchantGrant {
        enchant: enchant_id,
        skill_gained,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::enchants::EnchantId;
    use crate::inventory::Inventory;
    use crate::item::ItemId;
    use crate::professions::skill::ProfessionSkills;

    fn add_sword(inv: &mut Inventory) -> u64 {
        inv.try_add(ItemStack {
            item: ItemId::CopperShortsword,
            count: 1,
        })
        .unwrap();
        inv.instances[0].id
    }

    fn add_chest(inv: &mut Inventory) -> u64 {
        inv.try_add(ItemStack {
            item: ItemId::CopperChainVest,
            count: 1,
        })
        .unwrap();
        inv.instances[0].id
    }

    #[test]
    fn disenchant_common_sword_yields_one_dust_and_destroys_item() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_sword(&mut inv);
        let mut skills = ProfessionSkills::default();

        evaluate_disenchant(instance, &inv, false).unwrap();
        let grant = complete_disenchant(instance, &mut inv, &mut skills, false).unwrap();

        assert_eq!(grant.reagents.len(), 1);
        assert_eq!(grant.reagents[0].item, ItemId::ArcaneDust);
        assert_eq!(grant.reagents[0].count, 1);
        assert_eq!(inv.count(ItemId::ArcaneDust), 1);
        assert!(inv.instance(instance).is_none());
        assert_eq!(skills.get(ProfessionId::Enchanting), 2);
    }

    #[test]
    fn apply_without_confirm_on_already_enchanted_denies() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_chest(&mut inv);
        inv.try_add(ItemStack {
            item: ItemId::ArcaneDust,
            count: 10,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::ArcaneEssence,
            count: 10,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();

        inv.instance_mut(instance).unwrap().enchant = Some(EnchantId::BracerMinorHealth);

        let err = evaluate_apply_enchant(
            instance,
            EnchantId::ChestMinorStamina,
            false,
            &inv,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::AlreadyEnchanted);

        let dust_before = inv.count(ItemId::ArcaneDust);
        let essence_before = inv.count(ItemId::ArcaneEssence);
        let err = complete_apply_enchant(
            instance,
            EnchantId::ChestMinorStamina,
            false,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::AlreadyEnchanted);
        assert_eq!(inv.count(ItemId::ArcaneDust), dust_before);
        assert_eq!(inv.count(ItemId::ArcaneEssence), essence_before);
        assert_eq!(
            inv.instance(instance).unwrap().enchant,
            Some(EnchantId::BracerMinorHealth)
        );
    }

    #[test]
    fn same_enchant_id_denies_even_with_confirm() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_chest(&mut inv);
        inv.try_add(ItemStack {
            item: ItemId::ArcaneDust,
            count: 10,
        })
        .unwrap();
        inv.try_add(ItemStack {
            item: ItemId::ArcaneEssence,
            count: 10,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();

        complete_apply_enchant(
            instance,
            EnchantId::ChestMinorStamina,
            false,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap();

        let err = evaluate_apply_enchant(
            instance,
            EnchantId::ChestMinorStamina,
            true,
            &inv,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::SameEnchant);

        let dust_before = inv.count(ItemId::ArcaneDust);
        let essence_before = inv.count(ItemId::ArcaneEssence);
        let err = complete_apply_enchant(
            instance,
            EnchantId::ChestMinorStamina,
            true,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::SameEnchant);
        assert_eq!(inv.count(ItemId::ArcaneDust), dust_before);
        assert_eq!(inv.count(ItemId::ArcaneEssence), essence_before);
        assert_eq!(
            inv.instance(instance).unwrap().enchant,
            Some(EnchantId::ChestMinorStamina)
        );
    }

    #[test]
    fn wrong_slot_bracer_on_chest_denies() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_chest(&mut inv);
        inv.try_add(ItemStack {
            item: ItemId::ArcaneDust,
            count: 10,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();

        let err = evaluate_apply_enchant(
            instance,
            EnchantId::BracerMinorHealth,
            false,
            &inv,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::WrongSlot);

        let dust_before = inv.count(ItemId::ArcaneDust);
        let err = complete_apply_enchant(
            instance,
            EnchantId::BracerMinorHealth,
            false,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::WrongSlot);
        assert_eq!(inv.count(ItemId::ArcaneDust), dust_before);
        assert!(inv.instance(instance).unwrap().enchant.is_none());
    }

    #[test]
    fn missing_reagents_does_not_destroy_item() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_sword(&mut inv);
        let mut skills = ProfessionSkills::default();

        let err = complete_apply_enchant(
            instance,
            EnchantId::WeaponMinorMight,
            false,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap_err();
        assert_eq!(err, DenyReason::MissingReagents);
        assert!(inv.instance(instance).is_some());
    }

    #[test]
    fn apply_enchant_gains_skill() {
        let mut inv = Inventory::with_capacity(4);
        let instance = add_sword(&mut inv);
        inv.try_add(ItemStack {
            item: ItemId::ArcaneDust,
            count: 10,
        })
        .unwrap();
        let mut skills = ProfessionSkills::default();

        let grant = complete_apply_enchant(
            instance,
            EnchantId::WeaponMinorMight,
            false,
            &mut inv,
            &mut skills,
            false,
        )
        .unwrap();

        assert_eq!(grant.enchant, EnchantId::WeaponMinorMight);
        assert_eq!(skills.get(ProfessionId::Enchanting), 2);
        assert_eq!(
            inv.instance(instance).unwrap().enchant,
            Some(EnchantId::WeaponMinorMight)
        );
    }
}
