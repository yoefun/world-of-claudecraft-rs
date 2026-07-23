//! Shared wire / host types between sim, client, and server.

use serde::{Deserialize, Serialize};

/// Stable entity identifier within one `Sim` instance.
pub type EntityId = u32;

/// Protocol revision for snapshot / WS envelopes (0.1 was implicit rev 1).
/// Kept at 2: Wave 1 death/aura/party fields are additive with `#[serde(default)]`.
pub const PROTOCOL_REV: u32 = 2;

/// Fixed sim rate matching upstream World of ClaudeCraft.
pub const TICK_RATE: u32 = 20;
pub const DT: f32 = 1.0 / TICK_RATE as f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Player,
    Mob,
    Npc,
    Loot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilitySlot {
    /// Class primary ability.
    Primary = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InteractAction {
    Talk,
    AcceptQuest { quest_id: String },
    TurnInQuest { quest_id: String },
    Buy { item_id: String, count: u32 },
    Sell { bag_slot: u8, count: u32 },
    Equip { bag_slot: u8 },
    Unequip { equip_slot: EquipSlot },
    /// Use a bag item (consumable heal, etc.). Additive Wave 0.3.
    UseItem { bag_slot: u8 },
    LootCorpse { target_id: EntityId },
    CloseVendor,
}

/// Per-tick intent from a local or remote player.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct PlayerIntent {
    /// Forward/back wish in [-1, 1] (camera-relative on the client).
    pub move_x: f32,
    /// Strafe wish in [-1, 1].
    pub move_z: f32,
    /// Desired yaw in radians (world space).
    pub facing: f32,
    /// Start/continue auto-attack against `target_id`.
    pub attack: bool,
    /// Fire ability on this slot (if ready).
    pub ability: Option<AbilitySlot>,
    /// Selected target (mob or none).
    pub target_id: Option<EntityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub kind: EntityKind,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub hp: f32,
    pub hp_max: f32,
    pub level: u32,
    pub name: String,
    /// Class resource (rage/mana/energy); unused for mobs/loot.
    pub resource: f32,
    pub resource_max: f32,
    pub alive: bool,
    #[serde(default)]
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvSlotSnapshot {
    pub item_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EquipmentSnapshot {
    pub main_hand: Option<String>,
    pub off_hand: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    pub chest: Option<String>,
    #[serde(default)]
    pub legs: Option<String>,
    #[serde(default)]
    pub feet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestLogEntry {
    pub quest_id: String,
    /// "active" | "ready" | "completed"
    pub state: String,
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorOfferSnapshot {
    pub item_id: String,
    pub count: u32,
    pub price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorSnapshot {
    pub npc_id: EntityId,
    pub npc_name: String,
    pub stock: Vec<VendorOfferSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProgress {
    pub xp: u32,
    pub xp_to_level: u32,
    pub level: u32,
    pub copper: u32,
    /// Deprecated 0.1 stub; prefer `inventory`.
    #[serde(default)]
    pub bag_item: Option<String>,
    #[serde(default)]
    pub class_id: String,
    #[serde(default)]
    pub resource_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AuraSnapshot {
    pub id: String,
    pub remaining: f32,
    pub stacks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CastSnapshot {
    /// Ability currently being cast.
    pub ability_id: String,
    /// Cast progress in \[0, 1\].
    pub progress: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSnapshot {
    pub tick: u64,
    pub player_id: EntityId,
    pub entities: Vec<EntitySnapshot>,
    pub progress: PlayerProgress,
    pub target_id: Option<EntityId>,
    pub ability_ready: bool,
    pub ability_cooldown: f32,
    #[serde(default = "default_protocol_rev")]
    pub protocol_rev: u32,
    #[serde(default)]
    pub inventory: Vec<InvSlotSnapshot>,
    #[serde(default)]
    pub equipment: EquipmentSnapshot,
    #[serde(default)]
    pub quest_log: Vec<QuestLogEntry>,
    #[serde(default)]
    pub open_vendor: Option<VendorSnapshot>,
    #[serde(default)]
    pub ability_name: String,
    /// Active auras on the local player (Wave 1).
    #[serde(default)]
    pub auras: Vec<AuraSnapshot>,
    /// In-progress cast bar, if any.
    #[serde(default)]
    pub cast: Option<CastSnapshot>,
    /// True when the local player is dead.
    #[serde(default)]
    pub is_dead: bool,
    /// Party membership, if any.
    #[serde(default)]
    pub party_id: Option<u32>,
}

fn default_protocol_rev() -> u32 {
    PROTOCOL_REV
}

impl Default for PlayerProgress {
    fn default() -> Self {
        Self {
            xp: 0,
            xp_to_level: 0,
            level: 1,
            copper: 0,
            bag_item: None,
            class_id: String::new(),
            resource_type: String::new(),
        }
    }
}

impl Default for TickSnapshot {
    fn default() -> Self {
        Self {
            tick: 0,
            player_id: 0,
            entities: Vec::new(),
            progress: PlayerProgress::default(),
            target_id: None,
            ability_ready: false,
            ability_cooldown: 0.0,
            protocol_rev: PROTOCOL_REV,
            inventory: Vec::new(),
            equipment: EquipmentSnapshot::default(),
            quest_log: Vec::new(),
            open_vendor: None,
            ability_name: String::new(),
            auras: Vec::new(),
            cast: None,
            is_dead: false,
            party_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimEvent {
    Damage {
        source: EntityId,
        target: EntityId,
        amount: f32,
        ability: Option<String>,
    },
    Kill {
        killer: EntityId,
        victim: EntityId,
        victim_name: String,
    },
    Loot {
        player: EntityId,
        copper: u32,
        item: Option<String>,
    },
    LevelUp {
        player: EntityId,
        level: u32,
    },
    Toast {
        message: String,
    },
    QuestAccepted {
        player: EntityId,
        quest_id: String,
    },
    QuestProgress {
        player: EntityId,
        quest_id: String,
        objective_index: u32,
        current: u32,
        required: u32,
        text: String,
    },
    QuestCompleted {
        player: EntityId,
        quest_id: String,
    },
    ItemGained {
        player: EntityId,
        item_id: String,
        count: u32,
    },
    ItemLost {
        player: EntityId,
        item_id: String,
        count: u32,
    },
    Equipped {
        player: EntityId,
        item_id: String,
        slot: EquipSlot,
    },
    VendorOpen {
        player: EntityId,
        npc_id: EntityId,
    },
    NpcDialog {
        player: EntityId,
        npc_id: EntityId,
        text: String,
    },
    PlayerDied {
        player: EntityId,
    },
    AuraApplied {
        player: EntityId,
        id: String,
        remaining: f32,
        stacks: u32,
    },
}

/// Host facade shared by offline Bevy and online server.
pub trait WorldHost {
    fn push_intent(&mut self, player_id: EntityId, intent: PlayerIntent);
    fn interact(&mut self, player_id: EntityId, target_id: EntityId, action: InteractAction);
    fn tick_once(&mut self) -> (TickSnapshot, Vec<SimEvent>);
    fn snapshot_for(&self, player_id: EntityId) -> TickSnapshot;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMsg {
    Hello {
        name: String,
        class_id: String,
    },
    Intent(PlayerIntent),
    Interact {
        target_id: EntityId,
        action: InteractAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMsg {
    Welcome {
        player_id: EntityId,
        protocol_rev: u32,
    },
    Snapshot(Box<TickSnapshot>),
    Events {
        events: Vec<SimEvent>,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interact_action_roundtrip() {
        let actions = vec![
            InteractAction::Talk,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
            InteractAction::TurnInQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
            InteractAction::Buy {
                item_id: "travelers_ration".into(),
                count: 1,
            },
            InteractAction::Sell {
                bag_slot: 0,
                count: 1,
            },
            InteractAction::Equip { bag_slot: 0 },
            InteractAction::Unequip {
                equip_slot: EquipSlot::MainHand,
            },
            InteractAction::UseItem { bag_slot: 2 },
            InteractAction::LootCorpse { target_id: 3 },
            InteractAction::CloseVendor,
        ];
        for a in actions {
            let v = serde_json::to_value(&a).unwrap();
            let back: InteractAction = serde_json::from_value(v).unwrap();
            assert_eq!(format!("{back:?}"), format!("{a:?}"));
        }
    }

    #[test]
    fn ws_hello_roundtrip() {
        let msg = WsClientMsg::Hello {
            name: "Ada".into(),
            class_id: "mage".into(),
        };
        let s = serde_json::to_string(&msg).unwrap();
        let back: WsClientMsg = serde_json::from_str(&s).unwrap();
        match back {
            WsClientMsg::Hello { name, class_id } => {
                assert_eq!(name, "Ada");
                assert_eq!(class_id, "mage");
            }
            _ => panic!("expected Hello"),
        }
    }

    fn minimal_tick_json() -> &'static str {
        r#"{
            "tick": 1,
            "player_id": 1,
            "entities": [],
            "progress": {
                "xp": 0,
                "xp_to_level": 100,
                "level": 1,
                "copper": 0
            },
            "target_id": null,
            "ability_ready": true,
            "ability_cooldown": 0.0
        }"#
    }

    #[test]
    fn tick_snapshot_old_json_defaults_new_fields() {
        let snap: TickSnapshot = serde_json::from_str(minimal_tick_json()).unwrap();
        assert!(snap.auras.is_empty());
        assert!(snap.cast.is_none());
        assert!(!snap.is_dead);
        assert!(snap.party_id.is_none());
        assert_eq!(snap.protocol_rev, PROTOCOL_REV);
    }

    #[test]
    fn tick_snapshot_death_aura_party_roundtrip() {
        let snap = TickSnapshot {
            tick: 42,
            player_id: 7,
            entities: vec![],
            progress: PlayerProgress {
                xp: 10,
                xp_to_level: 100,
                level: 2,
                copper: 5,
                bag_item: None,
                class_id: "warrior".into(),
                resource_type: "rage".into(),
            },
            target_id: None,
            ability_ready: false,
            ability_cooldown: 0.5,
            protocol_rev: PROTOCOL_REV,
            inventory: vec![],
            equipment: EquipmentSnapshot::default(),
            quest_log: vec![],
            open_vendor: None,
            ability_name: "Strike".into(),
            auras: vec![AuraSnapshot {
                id: "blessing".into(),
                remaining: 12.5,
                stacks: 2,
            }],
            cast: Some(CastSnapshot {
                ability_id: "fireball".into(),
                progress: 0.35,
            }),
            is_dead: true,
            party_id: Some(3),
        };
        let s = serde_json::to_string(&snap).unwrap();
        let back: TickSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(back.auras.len(), 1);
        assert_eq!(back.auras[0].id, "blessing");
        assert!((back.auras[0].remaining - 12.5).abs() < f32::EPSILON);
        assert_eq!(back.auras[0].stacks, 2);
        let cast = back.cast.expect("cast present");
        assert_eq!(cast.ability_id, "fireball");
        assert!((cast.progress - 0.35).abs() < f32::EPSILON);
        assert!(back.is_dead);
        assert_eq!(back.party_id, Some(3));
    }

    #[test]
    fn sim_event_player_died_aura_applied_roundtrip() {
        let events = vec![
            SimEvent::PlayerDied { player: 9 },
            SimEvent::AuraApplied {
                player: 9,
                id: "regen".into(),
                remaining: 8.0,
                stacks: 1,
            },
        ];
        for e in events {
            let v = serde_json::to_value(&e).unwrap();
            let back: SimEvent = serde_json::from_value(v).unwrap();
            assert_eq!(back, e);
        }
    }
}
