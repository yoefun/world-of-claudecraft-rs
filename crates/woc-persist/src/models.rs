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
    #[serde(default = "default_zone_id")]
    pub zone_id: String,
    #[serde(default)]
    pub talent_points: u32,
    #[serde(default)]
    pub talents: Vec<TalentRankDto>,
    #[serde(default)]
    pub bank: Vec<Option<InvStackDto>>,
    #[serde(default)]
    pub bank_copper: u32,
    #[serde(default)]
    pub honor: u32,
    #[serde(default)]
    pub professions: Vec<ProfessionSkillDto>,
    #[serde(default)]
    pub pvp_flagged: bool,
    #[serde(default)]
    pub completed_deeds: Vec<String>,
    #[serde(default = "default_hearth_zone_id")]
    pub hearth_zone_id: String,
    #[serde(default = "default_hearth_x")]
    pub hearth_x: f32,
    #[serde(default = "default_hearth_z")]
    pub hearth_z: f32,
    #[serde(default)]
    pub hearth_ready_tick: u64,
    #[serde(default)]
    pub stance_id: String,
}

/// Fields updated on save (position / progression / bags).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterSave {
    pub level: u32,
    pub xp: u32,
    pub copper: u32,
    pub pos_x: f32,
    pub pos_z: f32,
    pub inventory: Vec<Option<InvStackDto>>,
    pub equipment: EquipmentDto,
    pub quests: Vec<QuestProgressDto>,
    #[serde(default = "default_zone_id")]
    pub zone_id: String,
    #[serde(default)]
    pub talent_points: u32,
    #[serde(default)]
    pub talents: Vec<TalentRankDto>,
    #[serde(default)]
    pub bank: Vec<Option<InvStackDto>>,
    #[serde(default)]
    pub bank_copper: u32,
    #[serde(default)]
    pub honor: u32,
    #[serde(default)]
    pub professions: Vec<ProfessionSkillDto>,
    #[serde(default)]
    pub pvp_flagged: bool,
    #[serde(default)]
    pub completed_deeds: Vec<String>,
    #[serde(default = "default_hearth_zone_id")]
    pub hearth_zone_id: String,
    #[serde(default = "default_hearth_x")]
    pub hearth_x: f32,
    #[serde(default = "default_hearth_z")]
    pub hearth_z: f32,
    #[serde(default)]
    pub hearth_ready_tick: u64,
    #[serde(default)]
    pub stance_id: String,
}

impl Default for CharacterSave {
    fn default() -> Self {
        Self {
            level: 0,
            xp: 0,
            copper: 0,
            pos_x: 0.0,
            pos_z: 0.0,
            inventory: Vec::new(),
            equipment: EquipmentDto::default(),
            quests: Vec::new(),
            zone_id: default_zone_id(),
            talent_points: 0,
            talents: Vec::new(),
            bank: Vec::new(),
            bank_copper: 0,
            honor: 0,
            professions: Vec::new(),
            pvp_flagged: false,
            completed_deeds: Vec::new(),
            hearth_zone_id: default_hearth_zone_id(),
            hearth_x: default_hearth_x(),
            hearth_z: default_hearth_z(),
            hearth_ready_tick: 0,
            stance_id: String::new(),
        }
    }
}

/// Completion state stored in the historical `quests_json` Postgres column.
///
/// The reader also accepts the legacy bare quest array so existing rows remain loadable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct CharacterCompletionDto {
    #[serde(default)]
    pub quests: Vec<QuestProgressDto>,
    #[serde(default = "default_zone_id")]
    pub zone_id: String,
    #[serde(default)]
    pub talent_points: u32,
    #[serde(default)]
    pub talents: Vec<TalentRankDto>,
    #[serde(default)]
    pub bank: Vec<Option<InvStackDto>>,
    #[serde(default)]
    pub bank_copper: u32,
    #[serde(default)]
    pub honor: u32,
    #[serde(default)]
    pub professions: Vec<ProfessionSkillDto>,
    #[serde(default)]
    pub pvp_flagged: bool,
    #[serde(default)]
    pub completed_deeds: Vec<String>,
    #[serde(default = "default_hearth_zone_id")]
    pub hearth_zone_id: String,
    #[serde(default = "default_hearth_x")]
    pub hearth_x: f32,
    #[serde(default = "default_hearth_z")]
    pub hearth_z: f32,
    #[serde(default)]
    pub hearth_ready_tick: u64,
    #[serde(default)]
    pub stance_id: String,
}

impl From<&CharacterSave> for CharacterCompletionDto {
    fn from(save: &CharacterSave) -> Self {
        Self {
            quests: save.quests.clone(),
            zone_id: save.zone_id.clone(),
            talent_points: save.talent_points,
            talents: save.talents.clone(),
            bank: save.bank.clone(),
            bank_copper: save.bank_copper,
            honor: save.honor,
            professions: save.professions.clone(),
            pvp_flagged: save.pvp_flagged,
            completed_deeds: save.completed_deeds.clone(),
            hearth_zone_id: save.hearth_zone_id.clone(),
            hearth_x: save.hearth_x,
            hearth_z: save.hearth_z,
            hearth_ready_tick: save.hearth_ready_tick,
            stance_id: save.stance_id.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredCompletionDto {
    Current(CharacterCompletionDto),
    Legacy(Vec<QuestProgressDto>),
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
            zone_id: self.zone_id.clone(),
            talent_points: self.talent_points,
            talents: self.talents.clone(),
            bank: self.bank.clone(),
            bank_copper: self.bank_copper,
            honor: self.honor,
            professions: self.professions.clone(),
            pvp_flagged: self.pvp_flagged,
            completed_deeds: self.completed_deeds.clone(),
            hearth_zone_id: self.hearth_zone_id.clone(),
            hearth_x: self.hearth_x,
            hearth_z: self.hearth_z,
            hearth_ready_tick: self.hearth_ready_tick,
            stance_id: self.stance_id.clone(),
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
        self.zone_id = save.zone_id;
        self.talent_points = save.talent_points;
        self.talents = save.talents;
        self.bank = save.bank;
        self.bank_copper = save.bank_copper;
        self.honor = save.honor;
        self.professions = save.professions;
        self.pvp_flagged = save.pvp_flagged;
        self.completed_deeds = save.completed_deeds;
        self.hearth_zone_id = save.hearth_zone_id;
        self.hearth_x = save.hearth_x;
        self.hearth_z = save.hearth_z;
        self.hearth_ready_tick = save.hearth_ready_tick;
        self.stance_id = save.stance_id;
    }
}

fn default_zone_id() -> String {
    "eastbrook".into()
}

fn default_hearth_zone_id() -> String {
    "eastbrook".into()
}

fn default_hearth_x() -> f32 {
    2.0
}

fn default_hearth_z() -> f32 {
    4.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvStackDto {
    pub item_id: String,
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enchant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TalentRankDto {
    pub talent_id: String,
    pub rank: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfessionSkillDto {
    pub id: String,
    pub skill: u32,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neck: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finger2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand_enchant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand_durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_hand_durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chest_durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs_durability: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet_durability: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestProgressDto {
    pub quest_id: String,
    pub state: String,
    #[serde(default)]
    pub counts: Vec<u32>,
    #[serde(default)]
    pub completed_tick: u64,
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

pub(crate) fn completion_to_json(save: &CharacterSave) -> Result<String, serde_json::Error> {
    serde_json::to_string(&CharacterCompletionDto::from(save))
}

pub(crate) fn completion_from_json(s: &str) -> Result<CharacterCompletionDto, serde_json::Error> {
    serde_json::from_str::<StoredCompletionDto>(s).map(|stored| match stored {
        StoredCompletionDto::Current(state) => state,
        StoredCompletionDto::Legacy(quests) => CharacterCompletionDto {
            quests,
            zone_id: default_zone_id(),
            talent_points: 0,
            talents: Vec::new(),
            bank: Vec::new(),
            bank_copper: 0,
            honor: 0,
            professions: Vec::new(),
            pvp_flagged: false,
            completed_deeds: Vec::new(),
            hearth_zone_id: default_hearth_zone_id(),
            hearth_x: default_hearth_x(),
            hearth_z: default_hearth_z(),
            hearth_ready_tick: 0,
            stance_id: String::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn old_character_json_uses_r4_defaults() {
        let character: Character = serde_json::from_value(json!({
            "id": Uuid::nil(),
            "account_id": Uuid::nil(),
            "name": "Aldric",
            "class_id": "warrior",
            "level": 3,
            "xp": 450,
            "copper": 12,
            "pos_x": 10.5,
            "pos_z": -2.0,
            "inventory": [],
            "equipment": {},
            "quests": []
        }))
        .unwrap();

        assert_eq!(character.zone_id, "eastbrook");
        assert_eq!(character.talent_points, 0);
        assert!(character.talents.is_empty());
        assert!(character.bank.is_empty());
        assert_eq!(character.honor, 0);
        assert!(character.professions.is_empty());
        assert!(!character.pvp_flagged);
        assert!(character.completed_deeds.is_empty());
        assert_eq!(character.hearth_zone_id, "eastbrook");
        assert_eq!(character.hearth_x, 2.0);
        assert_eq!(character.hearth_z, 4.0);
        assert_eq!(character.hearth_ready_tick, 0);
        assert!(character.stance_id.is_empty());
    }

    #[test]
    fn old_character_save_json_uses_r4_defaults() {
        let save: CharacterSave = serde_json::from_value(json!({
            "level": 3,
            "xp": 450,
            "copper": 12,
            "pos_x": 10.5,
            "pos_z": -2.0,
            "inventory": [],
            "equipment": {},
            "quests": []
        }))
        .unwrap();

        assert_eq!(save.zone_id, "eastbrook");
        assert_eq!(save.talent_points, 0);
        assert!(save.talents.is_empty());
        assert!(save.bank.is_empty());
        assert_eq!(save.honor, 0);
        assert!(save.professions.is_empty());
        assert!(!save.pvp_flagged);
        assert!(save.completed_deeds.is_empty());
        assert_eq!(save.hearth_zone_id, "eastbrook");
        assert_eq!(save.hearth_x, 2.0);
        assert_eq!(save.hearth_z, 4.0);
        assert_eq!(save.hearth_ready_tick, 0);
        assert!(save.stance_id.is_empty());
    }

    #[test]
    fn character_save_roundtrips_r4_completion_fields() {
        let mut character = Character {
            id: Uuid::nil(),
            account_id: Uuid::nil(),
            name: "Aldric".into(),
            class_id: "warrior".into(),
            level: 1,
            xp: 0,
            copper: 0,
            pos_x: 0.0,
            pos_z: 0.0,
            inventory: Vec::new(),
            equipment: EquipmentDto::default(),
            quests: Vec::new(),
            zone_id: "eastbrook".into(),
            talent_points: 0,
            talents: Vec::new(),
            bank: Vec::new(),
            bank_copper: 0,
            honor: 0,
            professions: Vec::new(),
            pvp_flagged: false,
            completed_deeds: Vec::new(),
            hearth_zone_id: "eastbrook".into(),
            hearth_x: 2.0,
            hearth_z: 4.0,
            hearth_ready_tick: 0,
            stance_id: String::new(),
        };
        let save = CharacterSave {
            zone_id: "eastfen".into(),
            talent_points: 2,
            talents: vec![TalentRankDto {
                talent_id: "shield_mastery".into(),
                rank: 3,
            }],
            bank: vec![Some(InvStackDto {
                item_id: "silverleaf".into(),
                count: 8,
                durability: None,
                enchant_id: None,
            })],
            bank_copper: 0,
            honor: 125,
            professions: vec![ProfessionSkillDto {
                id: "herbalism".into(),
                skill: 42,
            }],
            pvp_flagged: true,
            hearth_zone_id: "eastfen".into(),
            hearth_x: 12.0,
            hearth_z: 34.0,
            hearth_ready_tick: 77,
            ..Default::default()
        };

        character.apply_save(save.clone());

        assert_eq!(character.to_save(), save);
    }

    #[test]
    fn completion_json_reads_legacy_quest_array() {
        let state = completion_from_json(
            r#"[{"quest_id":"wolves_at_the_gate","state":"active","counts":[1]}]"#,
        )
        .unwrap();

        assert_eq!(state.quests.len(), 1);
        assert_eq!(state.zone_id, "eastbrook");
        assert_eq!(state.talent_points, 0);
        assert!(state.talents.is_empty());
        assert!(state.bank.is_empty());
        assert_eq!(state.honor, 0);
        assert!(state.professions.is_empty());
        assert!(!state.pvp_flagged);
        assert_eq!(state.hearth_zone_id, "eastbrook");
        assert_eq!(state.hearth_x, 2.0);
        assert_eq!(state.hearth_z, 4.0);
        assert_eq!(state.hearth_ready_tick, 0);
    }

    #[test]
    fn completion_json_roundtrips_r4_fields() {
        let save = CharacterSave {
            quests: vec![QuestProgressDto {
                quest_id: "q1".into(),
                state: "ready".into(),
                counts: vec![2],
                completed_tick: 0,
            }],
            zone_id: "eastfen".into(),
            talent_points: 2,
            talents: vec![TalentRankDto {
                talent_id: "shield_mastery".into(),
                rank: 3,
            }],
            bank: vec![Some(InvStackDto {
                item_id: "silverleaf".into(),
                count: 8,
                durability: None,
                enchant_id: None,
            })],
            bank_copper: 0,
            honor: 125,
            professions: vec![ProfessionSkillDto {
                id: "herbalism".into(),
                skill: 42,
            }],
            pvp_flagged: true,
            hearth_zone_id: "eastfen".into(),
            hearth_x: 12.0,
            hearth_z: 34.0,
            hearth_ready_tick: 77,
            ..Default::default()
        };

        let state = completion_from_json(&completion_to_json(&save).unwrap()).unwrap();

        assert_eq!(state, CharacterCompletionDto::from(&save));
    }
}
