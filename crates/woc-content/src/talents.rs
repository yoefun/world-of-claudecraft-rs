//! Talent definitions — class trees (framework depth).

#[derive(Debug, Clone)]
pub struct TalentDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Class id string, or `"*"` for any class.
    pub class_id: &'static str,
    pub tier: u32,
    pub max_rank: u32,
    /// Stable effect key for simulation wiring:
    /// - `damage_pct`: outgoing damage multiplier
    /// - `max_hp_pct`: maximum health multiplier
    /// - `armor_pct`: total armor multiplier
    /// - `armor_flat`: flat armor points
    /// - `resource_pct`: maximum class-resource multiplier
    /// - `cleave_targets_plus`: extra AoE max targets
    /// - `heal_pct`: outgoing heal multiplier
    /// - `crit_pct`: added crit chance
    ///
    /// Percentage effects contribute `effect_value * rank`; `armor_flat`
    /// contributes that many armor points per rank.
    pub effect: &'static str,
    pub effect_value: f32,
}

pub static TALENTS: &[TalentDef] = &[
    TalentDef {
        id: "warrior_cruelty",
        name: "Cruelty",
        class_id: "warrior",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.05,
    },
    TalentDef {
        id: "warrior_toughness",
        name: "Toughness",
        class_id: "warrior",
        tier: 1,
        max_rank: 5,
        effect: "armor_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "warrior_vitality",
        name: "Vitality",
        class_id: "warrior",
        tier: 2,
        max_rank: 5,
        effect: "max_hp_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "warrior_improved_cleave",
        name: "Improved Cleave",
        class_id: "warrior",
        tier: 1,
        max_rank: 1,
        effect: "cleave_targets_plus",
        effect_value: 1.0,
    },
    TalentDef {
        id: "mage_arcane_power",
        name: "Arcane Power",
        class_id: "mage",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.05,
    },
    TalentDef {
        id: "mage_arcane_fortitude",
        name: "Arcane Fortitude",
        class_id: "mage",
        tier: 1,
        max_rank: 5,
        effect: "max_hp_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "mage_arcane_mind",
        name: "Arcane Mind",
        class_id: "mage",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "mage_arcane_precision",
        name: "Arcane Precision",
        class_id: "mage",
        tier: 1,
        max_rank: 5,
        effect: "crit_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "hunter_lethal_shots",
        name: "Lethal Shots",
        class_id: "hunter",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "hunter_survivalist",
        name: "Survivalist",
        class_id: "hunter",
        tier: 1,
        max_rank: 5,
        effect: "max_hp_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "hunter_focused_instincts",
        name: "Focused Instincts",
        class_id: "hunter",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "hunter_killer_instinct",
        name: "Killer Instinct",
        class_id: "hunter",
        tier: 1,
        max_rank: 5,
        effect: "crit_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "warlock_demonic_power",
        name: "Demonic Power",
        class_id: "warlock",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "warlock_demonic_embrace",
        name: "Demonic Embrace",
        class_id: "warlock",
        tier: 1,
        max_rank: 5,
        effect: "max_hp_pct",
        effect_value: 0.025,
    },
    TalentDef {
        id: "warlock_fel_intellect",
        name: "Fel Intellect",
        class_id: "warlock",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "warlock_ruin",
        name: "Ruin",
        class_id: "warlock",
        tier: 1,
        max_rank: 5,
        effect: "crit_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "rogue_malice",
        name: "Malice",
        class_id: "rogue",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.05,
    },
    TalentDef {
        id: "rogue_lightning_reflexes",
        name: "Lightning Reflexes",
        class_id: "rogue",
        tier: 1,
        max_rank: 5,
        effect: "armor_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "rogue_vigor",
        name: "Vigor",
        class_id: "rogue",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "rogue_puncturing_wounds",
        name: "Puncturing Wounds",
        class_id: "rogue",
        tier: 1,
        max_rank: 5,
        effect: "crit_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "priest_holy_specialization",
        name: "Holy Specialization",
        class_id: "priest",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "priest_inner_fortitude",
        name: "Inner Fortitude",
        class_id: "priest",
        tier: 1,
        max_rank: 5,
        effect: "armor_flat",
        effect_value: 2.0,
    },
    TalentDef {
        id: "priest_mental_strength",
        name: "Mental Strength",
        class_id: "priest",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "priest_spiritual_healing",
        name: "Spiritual Healing",
        class_id: "priest",
        tier: 1,
        max_rank: 5,
        effect: "heal_pct",
        effect_value: 0.05,
    },
    TalentDef {
        id: "paladin_conviction",
        name: "Conviction",
        class_id: "paladin",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "paladin_sacred_duty",
        name: "Sacred Duty",
        class_id: "paladin",
        tier: 1,
        max_rank: 5,
        effect: "max_hp_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "paladin_shield_specialization",
        name: "Shield Specialization",
        class_id: "paladin",
        tier: 2,
        max_rank: 5,
        effect: "armor_flat",
        effect_value: 2.0,
    },
    TalentDef {
        id: "paladin_healing_light",
        name: "Healing Light",
        class_id: "paladin",
        tier: 1,
        max_rank: 5,
        effect: "heal_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "shaman_elemental_fury",
        name: "Elemental Fury",
        class_id: "shaman",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "shaman_ancestral_fortitude",
        name: "Ancestral Fortitude",
        class_id: "shaman",
        tier: 1,
        max_rank: 5,
        effect: "armor_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "shaman_tidal_focus",
        name: "Tidal Focus",
        class_id: "shaman",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.025,
    },
    TalentDef {
        id: "shaman_elemental_precision",
        name: "Elemental Precision",
        class_id: "shaman",
        tier: 1,
        max_rank: 5,
        effect: "crit_pct",
        effect_value: 0.02,
    },
    TalentDef {
        id: "druid_naturalist",
        name: "Naturalist",
        class_id: "druid",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
    },
    TalentDef {
        id: "druid_thick_hide",
        name: "Thick Hide",
        class_id: "druid",
        tier: 1,
        max_rank: 5,
        effect: "armor_pct",
        effect_value: 0.03,
    },
    TalentDef {
        id: "druid_heart_of_the_wild",
        name: "Heart of the Wild",
        class_id: "druid",
        tier: 2,
        max_rank: 5,
        effect: "resource_pct",
        effect_value: 0.025,
    },
    TalentDef {
        id: "druid_gift_of_nature",
        name: "Gift of Nature",
        class_id: "druid",
        tier: 1,
        max_rank: 5,
        effect: "heal_pct",
        effect_value: 0.04,
    },
];

pub fn talent(id: &str) -> Option<&'static TalentDef> {
    TALENTS.iter().find(|t| t.id == id)
}

pub fn talents_for_class(class_id: &str) -> impl Iterator<Item = &'static TalentDef> + '_ {
    TALENTS
        .iter()
        .filter(move |t| t.class_id == class_id || t.class_id == "*")
}

/// Points that must be spent in lower tiers before a talent of `tier` unlocks.
/// Tier 1 is free; each subsequent tier needs `POINTS_PER_TIER` in prior tiers.
pub const POINTS_PER_TIER: u32 = 5;

/// Human-readable one-line effect for a talent rank (or per-rank if `rank` is 0).
pub fn format_talent_effect(def: &TalentDef, rank: u32) -> String {
    let r = rank.max(1) as f32;
    let per = def.effect_value;
    match def.effect {
        "damage_pct" => format!(
            "+{:.0}% damage{}",
            per * r * 100.0,
            if rank == 0 { "/rank" } else { "" }
        ),
        "max_hp_pct" => format!(
            "+{:.0}% max HP{}",
            per * r * 100.0,
            if rank == 0 { "/rank" } else { "" }
        ),
        "armor_pct" => format!(
            "+{:.0}% armor{}",
            per * r * 100.0,
            if rank == 0 { "/rank" } else { "" }
        ),
        "armor_flat" => format!(
            "+{:.0} armor{}",
            per * r,
            if rank == 0 { "/rank" } else { "" }
        ),
        "resource_pct" => {
            format!(
                "+{:.0}% resource{}",
                per * r * 100.0,
                if rank == 0 { "/rank" } else { "" }
            )
        }
        "cleave_targets_plus" => format!(
            "+{:.0} cleave target{}{}",
            per * r,
            if per * r == 1.0 { "" } else { "s" },
            if rank == 0 { "/rank" } else { "" }
        ),
        "heal_pct" => format!(
            "+{:.0}% healing{}",
            per * r * 100.0,
            if rank == 0 { "/rank" } else { "" }
        ),
        "crit_pct" => format!(
            "+{:.0}% crit chance{}",
            per * r * 100.0,
            if rank == 0 { "/rank" } else { "" }
        ),
        other => format!("{other} ×{r}"),
    }
}

/// Total points spent in talents with `tier < max_tier` for `class_id`.
pub fn points_spent_below_tier(class_id: &str, ranks: &[(String, u32)], max_tier: u32) -> u32 {
    ranks
        .iter()
        .filter_map(|(id, rank)| {
            let def = talent(id)?;
            if def.class_id != class_id && def.class_id != "*" {
                return None;
            }
            (def.tier < max_tier).then_some(*rank)
        })
        .sum()
}

/// Whether `def` is unlocked given current talent ranks for the player's class.
pub fn talent_tier_unlocked(class_id: &str, ranks: &[(String, u32)], def: &TalentDef) -> bool {
    if def.tier <= 1 {
        return true;
    }
    let needed = (def.tier - 1) * POINTS_PER_TIER;
    points_spent_below_tier(class_id, ranks, def.tier) >= needed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CLASSES;

    #[test]
    fn every_class_has_a_tiered_talent_tree() {
        for class in CLASSES {
            let talents = talents_for_class(class.id.as_str()).collect::<Vec<_>>();
            assert!(
                talents.len() >= 4,
                "{} needs at least four talents, got {}",
                class.name,
                talents.len()
            );
            assert!(
                talents.iter().any(|talent| talent.tier == 1),
                "{} needs a tier 1 talent",
                class.name
            );
            assert!(
                talents.iter().any(|talent| talent.tier == 2),
                "{} needs a tier 2 talent",
                class.name
            );
        }
    }

    #[test]
    fn talent_effects_use_known_names() {
        const KNOWN_EFFECTS: &[&str] = &[
            "damage_pct",
            "max_hp_pct",
            "armor_pct",
            "armor_flat",
            "resource_pct",
            "cleave_targets_plus",
            "heal_pct",
            "crit_pct",
        ];

        for talent in TALENTS {
            assert!(
                KNOWN_EFFECTS.contains(&talent.effect),
                "talent {} has unknown effect {}",
                talent.id,
                talent.effect
            );
        }
    }

    #[test]
    fn tier_two_requires_points_in_tier_one() {
        let empty: Vec<(String, u32)> = vec![];
        let tier2 = talents_for_class("warrior")
            .find(|t| t.tier == 2)
            .expect("warrior tier 2");
        assert!(!talent_tier_unlocked("warrior", &empty, tier2));

        let ranks = vec![("warrior_cruelty".into(), POINTS_PER_TIER)];
        assert!(talent_tier_unlocked("warrior", &ranks, tier2));
        assert!(format_talent_effect(tier2, 1).contains("HP"));
    }

    #[test]
    fn every_class_has_an_ability_mod_talent() {
        const ABILITY_MODS: &[&str] = &["cleave_targets_plus", "heal_pct", "crit_pct"];
        for class in CLASSES {
            let talents = talents_for_class(class.id.as_str()).collect::<Vec<_>>();
            assert!(
                talents.iter().any(|t| ABILITY_MODS.contains(&t.effect)),
                "{} needs an ability-mod talent",
                class.name
            );
        }
        let cleave = talent("warrior_improved_cleave").expect("improved cleave");
        assert_eq!(cleave.tier, 1);
        assert!(format_talent_effect(cleave, 1).contains("cleave"));
    }
}
