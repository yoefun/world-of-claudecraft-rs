//! Ability definitions for framework starter kits.

#[derive(Debug, Clone)]
pub struct AbilityDef {
    pub id: &'static str,
    pub name: &'static str,
    pub damage: f32,
    pub cost: f32,
    pub cooldown: f32,
    pub range: f32,
}

pub static ABILITIES: &[AbilityDef] = &[
    AbilityDef {
        id: "heroic_strike",
        name: "Heroic Strike",
        damage: 28.0,
        cost: 15.0,
        cooldown: 3.0,
        range: 3.0,
    },
    AbilityDef {
        id: "crusader_strike",
        name: "Crusader Strike",
        damage: 26.0,
        cost: 20.0,
        cooldown: 3.0,
        range: 3.0,
    },
    AbilityDef {
        id: "arcane_shot",
        name: "Arcane Shot",
        damage: 24.0,
        cost: 25.0,
        cooldown: 2.5,
        range: 18.0,
    },
    AbilityDef {
        id: "sinister_strike",
        name: "Sinister Strike",
        damage: 22.0,
        cost: 30.0,
        cooldown: 1.5,
        range: 3.0,
    },
    AbilityDef {
        id: "smite",
        name: "Smite",
        damage: 25.0,
        cost: 25.0,
        cooldown: 2.0,
        range: 18.0,
    },
    AbilityDef {
        id: "lightning_bolt",
        name: "Lightning Bolt",
        damage: 27.0,
        cost: 30.0,
        cooldown: 2.5,
        range: 18.0,
    },
    AbilityDef {
        id: "fireball",
        name: "Fireball",
        damage: 30.0,
        cost: 35.0,
        cooldown: 2.5,
        range: 20.0,
    },
    AbilityDef {
        id: "shadow_bolt",
        name: "Shadow Bolt",
        damage: 28.0,
        cost: 30.0,
        cooldown: 2.5,
        range: 18.0,
    },
    AbilityDef {
        id: "wrath",
        name: "Wrath",
        damage: 26.0,
        cost: 25.0,
        cooldown: 2.0,
        range: 18.0,
    },
];

pub fn ability(id: &str) -> Option<&'static AbilityDef> {
    ABILITIES.iter().find(|a| a.id == id)
}
