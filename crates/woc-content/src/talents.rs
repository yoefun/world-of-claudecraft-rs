//! Talent definitions — class trees (framework depth).

#[derive(Debug, Clone)]
pub struct TalentDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Class id string, or `"*"` for any class.
    pub class_id: &'static str,
    pub tier: u32,
    pub max_rank: u32,
    /// Effect kind: `damage_pct` adds `effect_value * rank` to outgoing damage.
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
        id: "mage_arcane_power",
        name: "Arcane Power",
        class_id: "mage",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.05,
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
        id: "warlock_demonic_power",
        name: "Demonic Power",
        class_id: "warlock",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
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
        id: "priest_holy_specialization",
        name: "Holy Specialization",
        class_id: "priest",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
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
        id: "shaman_elemental_fury",
        name: "Elemental Fury",
        class_id: "shaman",
        tier: 1,
        max_rank: 5,
        effect: "damage_pct",
        effect_value: 0.04,
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
];

pub fn talent(id: &str) -> Option<&'static TalentDef> {
    TALENTS.iter().find(|t| t.id == id)
}

pub fn talents_for_class(class_id: &str) -> impl Iterator<Item = &'static TalentDef> + '_ {
    TALENTS
        .iter()
        .filter(move |t| t.class_id == class_id || t.class_id == "*")
}
