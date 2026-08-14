//! Player class definitions (multi-ability kits).

use crate::abilities::{ability, AbilityDef};
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

/// One action-bar binding: key/slot `1..=5` → ability id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassKitEntry {
    /// Matches `AbilitySlot` discriminants: 1=Primary, 2=Slot2, … 5=Slot5.
    pub slot: u8,
    pub ability_id: &'static str,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: PlayerClass,
    pub name: &'static str,
    pub resource_type: ResourceType,
    pub base_hp: f32,
    pub resource_max: f32,
    pub primary_ability: &'static str,
    /// Action-bar kit (≥4 entries). Slot 1 must equal `primary_ability`.
    pub kit: &'static [ClassKitEntry],
    pub start_weapon: &'static str,
    pub start_chest: &'static str,
    pub start_items: &'static [(&'static str, u32)],
    pub attack_power: f32,
}

const RATIONS: &[(&str, u32)] = &[("baked_bread", 5)];
const RATIONS_MANA: &[(&str, u32)] = &[("baked_bread", 5), ("spring_water", 5)];

const WARRIOR_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "heroic_strike",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "cleave",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "execute",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "taunt",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "charge",
    },
];

const PALADIN_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "crusader_strike",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "judgment",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "holy_shock",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "holy_light",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "hammer_of_justice",
    },
];

const HUNTER_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "arcane_shot",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "serpent_sting",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "multi_shot",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "concussive_shot",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "aspect_of_the_hawk",
    },
];

const ROGUE_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "sinister_strike",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "eviscerate",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "cheap_shot",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "kick",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "sprint",
    },
];

const PRIEST_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "smite",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "holy_fire",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "shadow_word_pain",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "flash_heal",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "power_word_shield",
    },
];

const SHAMAN_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "lightning_bolt",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "earth_shock",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "lava_burst",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "healing_wave",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "lightning_shield",
    },
];

const MAGE_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "fireball",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "frostbolt",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "counterspell",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "frost_nova",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "blink",
    },
];

const WARLOCK_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "shadow_bolt",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "corruption",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "incinerate",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "life_tap",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "fear",
    },
];

const DRUID_KIT: &[ClassKitEntry] = &[
    ClassKitEntry {
        slot: 1,
        ability_id: "wrath",
    },
    ClassKitEntry {
        slot: 2,
        ability_id: "moonfire",
    },
    ClassKitEntry {
        slot: 3,
        ability_id: "starfire",
    },
    ClassKitEntry {
        slot: 4,
        ability_id: "rejuvenation",
    },
    ClassKitEntry {
        slot: 5,
        ability_id: "healing_touch",
    },
];

pub static CLASSES: &[ClassDef] = &[
    ClassDef {
        id: PlayerClass::Warrior,
        name: "Warrior",
        resource_type: ResourceType::Rage,
        base_hp: 120.0,
        resource_max: 100.0,
        primary_ability: "heroic_strike",
        kit: WARRIOR_KIT,
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
        kit: PALADIN_KIT,
        start_weapon: "worn_mace",
        start_chest: "recruit_tunic",
        start_items: RATIONS_MANA,
        attack_power: 16.0,
    },
    ClassDef {
        id: PlayerClass::Hunter,
        name: "Hunter",
        resource_type: ResourceType::Mana,
        base_hp: 100.0,
        resource_max: 100.0,
        primary_ability: "arcane_shot",
        kit: HUNTER_KIT,
        start_weapon: "worn_bow",
        start_chest: "recruit_tunic",
        start_items: RATIONS_MANA,
        attack_power: 14.0,
    },
    ClassDef {
        id: PlayerClass::Rogue,
        name: "Rogue",
        resource_type: ResourceType::Energy,
        base_hp: 95.0,
        resource_max: 100.0,
        primary_ability: "sinister_strike",
        kit: ROGUE_KIT,
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
        kit: PRIEST_KIT,
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
        kit: SHAMAN_KIT,
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
        kit: MAGE_KIT,
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
        kit: WARLOCK_KIT,
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
        kit: DRUID_KIT,
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

/// Resolve the ability bound to action-bar slot `1..=5` for a class.
pub fn class_ability_for_slot(class: PlayerClass, slot: u8) -> Option<&'static AbilityDef> {
    let entry = class_def(class).kit.iter().find(|e| e.slot == slot)?;
    ability(entry.ability_id)
}

/// Ability ids from the class kit that are unlocked at `level`.
pub fn known_abilities_at_level(class: PlayerClass, level: u32) -> Vec<&'static str> {
    class_def(class)
        .kit
        .iter()
        .filter_map(|e| {
            let def = ability(e.ability_id)?;
            (level >= def.min_level).then_some(e.ability_id)
        })
        .collect()
}
