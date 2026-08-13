use std::collections::BTreeMap;

use crate::content::enchants::EnchantId;
use crate::content::nodes::node_by_id;
use crate::content::recipes::recipe_by_id;
use crate::gold::Gold;
use crate::inventory::{Inventory, ItemStack};
use crate::item::ItemId;
use crate::rng::Rng;
use crate::ticks_from_seconds;

use super::crafting::{complete_craft, evaluate_craft_admission};
use super::duration::{craft_cast_seconds, enchant_family_seconds, gather_cast_seconds};
use super::enchanting::{
    complete_apply_enchant, complete_disenchant, evaluate_apply_enchant, evaluate_disenchant,
};
use super::gathering::{complete_gather, start_gather_node};
use super::skill::{tier_for_skill, ProfessionSkills};
use super::skinning::{complete_skin, start_skin};
use super::tools::{best_tool_tier, profession_for_node};
use super::types::{
    Corpse, CorpseId, DenyReason, NodeId, RecipeId, Vec2,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveCast {
    Gather {
        node: NodeId,
        complete_tick: u64,
    },
    Skin {
        corpse: CorpseId,
        complete_tick: u64,
    },
    Craft {
        recipe: RecipeId,
        remaining: u16,
        complete_tick: u64,
    },
    Disenchant {
        instance: u64,
        complete_tick: u64,
    },
    ApplyEnchant {
        instance: u64,
        enchant: EnchantId,
        confirm: bool,
        complete_tick: u64,
    },
}

#[derive(Clone, Debug)]
pub struct ProfessionSession {
    pub tick: u64,
    pub pos: Vec2,
    pub gold: Gold,
    pub inventory: Inventory,
    pub skills: ProfessionSkills,
    pub node_ready: BTreeMap<NodeId, u64>,
    pub corpses: BTreeMap<CorpseId, Corpse>,
    pub cast: Option<ActiveCast>,
    pub last_masterwork: Option<RecipeId>,
    pub last_deny: Option<DenyReason>,
}

impl ProfessionSession {
    pub fn new_eastbrook() -> Self {
        let mut inventory = Inventory::with_capacity(16);
        inventory
            .try_add(ItemStack {
                item: ItemId::CopperPick,
                count: 1,
            })
            .expect("eastbrook kit fits");
        inventory
            .try_add(ItemStack {
                item: ItemId::CopperSickle,
                count: 1,
            })
            .expect("eastbrook kit fits");
        inventory
            .try_add(ItemStack {
                item: ItemId::SkinningKnife,
                count: 1,
            })
            .expect("eastbrook kit fits");
        inventory
            .try_add(ItemStack {
                item: ItemId::SmithingFlux,
                count: 2,
            })
            .expect("eastbrook kit fits");

        Self {
            tick: 0,
            pos: Vec2 { x: 0.0, z: 0.0 },
            gold: Gold { copper: 1000 },
            inventory,
            skills: ProfessionSkills::default(),
            node_ready: BTreeMap::new(),
            corpses: BTreeMap::new(),
            cast: None,
            last_masterwork: None,
            last_deny: None,
        }
    }

    pub fn advance(&mut self, ticks: u32) {
        self.tick = self.tick.saturating_add(u64::from(ticks));
    }

    pub fn start_gather(&mut self, node: NodeId) -> Result<(), DenyReason> {
        if self.cast.is_some() {
            return self.deny(DenyReason::Busy);
        }
        let ready_tick = self.node_ready_tick(node);
        let node_def = start_gather_node(
            self.pos,
            &self.inventory,
            node,
            ready_tick,
            self.tick,
            false,
        )?;
        let profession = profession_for_node(node_def.kind);
        let duration = gather_duration_ticks(
            &self.inventory,
            &self.skills,
            profession,
            node_def.tier,
            node_def.skill_req,
        );
        self.cast = Some(ActiveCast::Gather {
            node,
            complete_tick: self.tick + u64::from(duration),
        });
        self.last_deny = None;
        Ok(())
    }

    pub fn start_skin(&mut self, corpse_id: CorpseId) -> Result<(), DenyReason> {
        if self.cast.is_some() {
            return self.deny(DenyReason::Busy);
        }
        let corpse = self
            .corpses
            .get(&corpse_id)
            .ok_or(DenyReason::CorpseGone)?;
        start_skin(self.pos, &self.inventory, corpse, false)?;
        let duration = skin_duration_ticks(&self.inventory, &self.skills, corpse);
        self.cast = Some(ActiveCast::Skin {
            corpse: corpse_id,
            complete_tick: self.tick + u64::from(duration),
        });
        self.last_deny = None;
        Ok(())
    }

    pub fn start_craft(&mut self, recipe: RecipeId, count: u16) -> Result<(), DenyReason> {
        if self.cast.is_some() {
            return self.deny(DenyReason::Busy);
        }
        evaluate_craft_admission(
            recipe,
            count,
            self.pos,
            &self.inventory,
            &self.gold,
            false,
        )?;
        let recipe_def = recipe_by_id(recipe).expect("admission checked recipe");
        let duration = ticks_from_seconds(craft_cast_seconds(recipe_def.skill_req));
        self.cast = Some(ActiveCast::Craft {
            recipe,
            remaining: count,
            complete_tick: self.tick + u64::from(duration),
        });
        self.last_deny = None;
        Ok(())
    }

    pub fn start_disenchant(&mut self, instance: u64) -> Result<(), DenyReason> {
        if self.cast.is_some() {
            return self.deny(DenyReason::Busy);
        }
        evaluate_disenchant(instance, &self.inventory, false)?;
        let duration = ticks_from_seconds(enchant_family_seconds());
        self.cast = Some(ActiveCast::Disenchant {
            instance,
            complete_tick: self.tick + u64::from(duration),
        });
        self.last_deny = None;
        Ok(())
    }

    pub fn start_enchant(
        &mut self,
        instance: u64,
        enchant: EnchantId,
        confirm: bool,
    ) -> Result<(), DenyReason> {
        if self.cast.is_some() {
            return self.deny(DenyReason::Busy);
        }
        evaluate_apply_enchant(instance, enchant, confirm, &self.inventory, false)?;
        let duration = ticks_from_seconds(enchant_family_seconds());
        self.cast = Some(ActiveCast::ApplyEnchant {
            instance,
            enchant,
            confirm,
            complete_tick: self.tick + u64::from(duration),
        });
        self.last_deny = None;
        Ok(())
    }

    pub fn complete_ready(&mut self, rng: &mut impl Rng) -> Result<(), DenyReason> {
        let cast = match self.cast.as_ref() {
            Some(c) if self.tick >= complete_tick_of(c) => c.clone(),
            _ => return Ok(()),
        };

        match cast {
            ActiveCast::Gather { node, .. } => {
                let ready_tick = self.node_ready_tick(node);
                let node_def = node_by_id(node).ok_or(DenyReason::UnknownNode)?;
                let grant = complete_gather(
                    self.pos,
                    &mut self.inventory,
                    &mut self.skills,
                    node_def,
                    ready_tick,
                    self.tick,
                    rng,
                )?;
                self.node_ready.insert(node, grant.next_ready_tick);
                self.cast = None;
            }
            ActiveCast::Skin { corpse, .. } => {
                let corpse_state = self
                    .corpses
                    .get_mut(&corpse)
                    .ok_or(DenyReason::CorpseGone)?;
                complete_skin(
                    self.pos,
                    &mut self.inventory,
                    &mut self.skills,
                    corpse_state,
                    rng,
                )?;
                self.cast = None;
            }
            ActiveCast::Craft {
                recipe,
                remaining,
                ..
            } => {
                complete_craft(
                    recipe,
                    1,
                    self.pos,
                    &mut self.inventory,
                    &mut self.gold,
                    &mut self.skills,
                    false,
                    &mut self.last_masterwork,
                    rng,
                )?;
                let next_remaining = remaining - 1;
                if next_remaining > 0 {
                    evaluate_craft_admission(
                        recipe,
                        1,
                        self.pos,
                        &self.inventory,
                        &self.gold,
                        false,
                    )?;
                    let recipe_def = recipe_by_id(recipe).expect("admission checked recipe");
                    let duration = ticks_from_seconds(craft_cast_seconds(recipe_def.skill_req));
                    self.cast = Some(ActiveCast::Craft {
                        recipe,
                        remaining: next_remaining,
                        complete_tick: self.tick + u64::from(duration),
                    });
                } else {
                    self.cast = None;
                }
            }
            ActiveCast::Disenchant { instance, .. } => {
                complete_disenchant(instance, &mut self.inventory, &mut self.skills, false)?;
                self.cast = None;
            }
            ActiveCast::ApplyEnchant {
                instance,
                enchant,
                confirm,
                ..
            } => {
                complete_apply_enchant(
                    instance,
                    enchant,
                    confirm,
                    &mut self.inventory,
                    &mut self.skills,
                    false,
                )?;
                self.cast = None;
            }
        }
        self.last_deny = None;
        Ok(())
    }

    fn deny(&mut self, reason: DenyReason) -> Result<(), DenyReason> {
        self.last_deny = Some(reason);
        Err(reason)
    }

    fn node_ready_tick(&self, node: NodeId) -> u64 {
        self.node_ready.get(&node).copied().unwrap_or(0)
    }
}

fn complete_tick_of(cast: &ActiveCast) -> u64 {
    match cast {
        ActiveCast::Gather { complete_tick, .. }
        | ActiveCast::Skin { complete_tick, .. }
        | ActiveCast::Craft { complete_tick, .. }
        | ActiveCast::Disenchant { complete_tick, .. }
        | ActiveCast::ApplyEnchant { complete_tick, .. } => *complete_tick,
    }
}

fn gather_duration_ticks(
    inv: &Inventory,
    skills: &ProfessionSkills,
    profession: super::types::ProfessionId,
    target_tier: u8,
    skill_req: u16,
) -> u32 {
    let tool_tier = best_tool_tier(inv, profession).unwrap_or(0);
    let tool_tiers_above = tool_tier.saturating_sub(target_tier);
    let proficiency_bands_above = tier_for_skill(skills.get(profession))
        .saturating_sub(tier_for_skill(skill_req));
    ticks_from_seconds(gather_cast_seconds(
        tool_tiers_above,
        proficiency_bands_above,
    ))
}

fn skin_duration_ticks(inv: &Inventory, skills: &ProfessionSkills, corpse: &Corpse) -> u32 {
    gather_duration_ticks(
        inv,
        skills,
        super::types::ProfessionId::Skinning,
        corpse.tier,
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::professions::types::ProfessionId;
    use crate::rng::ScriptedRng;

    #[test]
    fn start_gather_while_busy_returns_busy() {
        let mut session = ProfessionSession::new_eastbrook();
        let node = node_by_id(NodeId(1)).unwrap();
        session.pos = node.pos;
        session.start_gather(NodeId(1)).unwrap();
        let err = session.start_gather(NodeId(1)).unwrap_err();
        assert_eq!(err, DenyReason::Busy);
    }

    #[test]
    fn complete_ready_finishes_due_gather() {
        let mut session = ProfessionSession::new_eastbrook();
        let node = node_by_id(NodeId(1)).unwrap();
        session.pos = node.pos;
        session.start_gather(NodeId(1)).unwrap();
        session.advance(60);
        let mut rng = ScriptedRng::from_seq(&[99, 99]);
        session.complete_ready(&mut rng).unwrap();
        assert!(session.cast.is_none());
        assert_eq!(session.skills.get(ProfessionId::Mining), 2);
        assert!(session.inventory.count(ItemId::CopperOre) > 0);
    }
}
