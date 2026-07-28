//! Player class definitions (framework starter kits).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerClass {
    Warrior,
    Paladin,
    Hunter,
    Rogue,
    Priest,
    Shaman,
    Mage,
    Warlock,
    Druid,
}

impl PlayerClass {
    pub const ALL: [PlayerClass; 9] = [
        PlayerClass::Warrior,
        PlayerClass::Paladin,
        PlayerClass::Hunter,
        PlayerClass::Rogue,
        PlayerClass::Priest,
        PlayerClass::Shaman,
        PlayerClass::Mage,
        PlayerClass::Warlock,
        PlayerClass::Druid,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PlayerClass::Warrior => "warrior",
            PlayerClass::Paladin => "paladin",
            PlayerClass::Hunter => "hunter",
            PlayerClass::Rogue => "rogue",
            PlayerClass::Priest => "priest",
            PlayerClass::Shaman => "shaman",
            PlayerClass::Mage => "mage",
            PlayerClass::Warlock => "warlock",
            PlayerClass::Druid => "druid",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Rage,
    Mana,
    Energy,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: PlayerClass,
    pub name: &'static str,
    pub resource_type: ResourceType,
    pub base_hp: f32,
    pub resource_max: f32,
    pub primary_ability: &'static str,
    pub start_weapon: &'static str,
    pub start_chest: &'static str,
    pub start_items: &'static [(&'static str, u32)],
    pub attack_power: f32,
}

const RATIONS: &[(&str, u32)] = &[("baked_bread", 5)];
const RATIONS_MANA: &[(&str, u32)] = &[("baked_bread", 5), ("spring_water", 5)];

pub static CLASSES: &[ClassDef] = &[
    ClassDef {
        id: PlayerClass::Warrior,
        name: "Warrior",
        resource_type: ResourceType::Rage,
        base_hp: 120.0,
        resource_max: 100.0,
        primary_ability: "heroic_strike",
        start_weapon: "worn_sword",
        start_chest: "recruit_tunic",
        start_items: RATIONS,
        attack_power: 18.0,
    },
    ClassDef {
        id: PlayerClass::Paladin,
        name: "Paladin",
        resource_type: ResourceType::Mana,
        base_hp: 115.0,
        resource_max: 100.0,
        primary_ability: "crusader_strike",
        start_weapon: "worn_mace",
        start_chest: "recruit_tunic",
        start_items: RATIONS_MANA,
        attack_power: 16.0,
    },
    ClassDef {
        id: PlayerClass::Hunter,
        name: "Hunter",
        resource_type: ResourceType::Energy,
        base_hp: 100.0,
        resource_max: 100.0,
        primary_ability: "arcane_shot",
        start_weapon: "worn_bow",
        start_chest: "recruit_tunic",
        start_items: RATIONS,
        attack_power: 14.0,
    },
    ClassDef {
        id: PlayerClass::Rogue,
        name: "Rogue",
        resource_type: ResourceType::Energy,
        base_hp: 95.0,
        resource_max: 100.0,
        primary_ability: "sinister_strike",
        start_weapon: "worn_dagger",
        start_chest: "recruit_tunic",
        start_items: RATIONS,
        attack_power: 15.0,
    },
    ClassDef {
        id: PlayerClass::Priest,
        name: "Priest",
        resource_type: ResourceType::Mana,
        base_hp: 90.0,
        resource_max: 120.0,
        primary_ability: "smite",
        start_weapon: "worn_staff",
        start_chest: "recruit_robe",
        start_items: RATIONS_MANA,
        attack_power: 12.0,
    },
    ClassDef {
        id: PlayerClass::Shaman,
        name: "Shaman",
        resource_type: ResourceType::Mana,
        base_hp: 105.0,
        resource_max: 110.0,
        primary_ability: "lightning_bolt",
        start_weapon: "worn_mace",
        start_chest: "recruit_tunic",
        start_items: RATIONS_MANA,
        attack_power: 13.0,
    },
    ClassDef {
        id: PlayerClass::Mage,
        name: "Mage",
        resource_type: ResourceType::Mana,
        base_hp: 85.0,
        resource_max: 140.0,
        primary_ability: "fireball",
        start_weapon: "worn_staff",
        start_chest: "recruit_robe",
        start_items: RATIONS_MANA,
        attack_power: 14.0,
    },
    ClassDef {
        id: PlayerClass::Warlock,
        name: "Warlock",
        resource_type: ResourceType::Mana,
        base_hp: 90.0,
        resource_max: 130.0,
        primary_ability: "shadow_bolt",
        start_weapon: "worn_staff",
        start_chest: "recruit_robe",
        start_items: RATIONS_MANA,
        attack_power: 14.0,
    },
    ClassDef {
        id: PlayerClass::Druid,
        name: "Druid",
        resource_type: ResourceType::Mana,
        base_hp: 100.0,
        resource_max: 120.0,
        primary_ability: "wrath",
        start_weapon: "worn_staff",
        start_chest: "recruit_robe",
        start_items: RATIONS_MANA,
        attack_power: 13.0,
    },
];

pub fn class_def(id: PlayerClass) -> &'static ClassDef {
    CLASSES
        .iter()
        .find(|c| c.id == id)
        .expect("every PlayerClass has a ClassDef")
}
