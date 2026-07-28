//! Domain models and JSON DTOs for persisted characters.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Durable character record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Character {
    pub id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub class_id: String,
    pub level: u32,
    pub xp: u32,
    pub copper: u32,
    pub pos_x: f32,
    pub pos_z: f32,
    pub inventory: Vec<Option<InvStackDto>>,
    pub equipment: EquipmentDto,
    pub quests: Vec<QuestProgressDto>,
}

/// Fields updated on save (position / progression / bags).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CharacterSave {
    pub level: u32,
    pub xp: u32,
    pub copper: u32,
    pub pos_x: f32,
    pub pos_z: f32,
    pub inventory: Vec<Option<InvStackDto>>,
    pub equipment: EquipmentDto,
    pub quests: Vec<QuestProgressDto>,
}

impl Character {
    pub fn to_save(&self) -> CharacterSave {
        CharacterSave {
            level: self.level,
            xp: self.xp,
            copper: self.copper,
            pos_x: self.pos_x,
            pos_z: self.pos_z,
            inventory: self.inventory.clone(),
            equipment: self.equipment.clone(),
            quests: self.quests.clone(),
        }
    }

    pub fn apply_save(&mut self, save: CharacterSave) {
        self.level = save.level;
        self.xp = save.xp;
        self.copper = save.copper;
        self.pos_x = save.pos_x;
        self.pos_z = save.pos_z;
        self.inventory = save.inventory;
        self.equipment = save.equipment;
        self.quests = save.quests;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvStackDto {
    pub item_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EquipmentDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_hand: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestProgressDto {
    pub quest_id: String,
    pub state: String,
    #[serde(default)]
    pub counts: Vec<u32>,
}

/// Public character summary for list endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub class_id: String,
    pub level: u32,
}

impl From<&Character> for CharacterSummary {
    fn from(c: &Character) -> Self {
        Self {
            id: c.id,
            name: c.name.clone(),
            class_id: c.class_id.clone(),
            level: c.level,
        }
    }
}

/// Serialize helpers used by backends and tests.
pub fn inventory_to_json(inventory: &[Option<InvStackDto>]) -> Result<String, serde_json::Error> {
    serde_json::to_string(inventory)
}

pub fn inventory_from_json(s: &str) -> Result<Vec<Option<InvStackDto>>, serde_json::Error> {
    serde_json::from_str(s)
}

pub fn equipment_to_json(equipment: &EquipmentDto) -> Result<String, serde_json::Error> {
    serde_json::to_string(equipment)
}

pub fn equipment_from_json(s: &str) -> Result<EquipmentDto, serde_json::Error> {
    serde_json::from_str(s)
}

pub fn quests_to_json(quests: &[QuestProgressDto]) -> Result<String, serde_json::Error> {
    serde_json::to_string(quests)
}

pub fn quests_from_json(s: &str) -> Result<Vec<QuestProgressDto>, serde_json::Error> {
    serde_json::from_str(s)
}
