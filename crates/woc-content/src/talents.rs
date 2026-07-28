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
];

pub fn talent(id: &str) -> Option<&'static TalentDef> {
    TALENTS.iter().find(|t| t.id == id)
}

pub fn talents_for_class(class_id: &str) -> impl Iterator<Item = &'static TalentDef> + '_ {
    TALENTS
        .iter()
        .filter(move |t| t.class_id == class_id || t.class_id == "*")
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
                talents.len() >= 2,
                "{} needs at least two talents, got {}",
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
}
