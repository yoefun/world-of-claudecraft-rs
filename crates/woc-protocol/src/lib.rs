//! Shared wire / host types between sim, client, and server.

use serde::{Deserialize, Serialize};

/// Stable entity identifier within one `Sim` instance.
pub type EntityId = u32;

/// Protocol revision for snapshot / WS envelopes (0.1 was implicit rev 1).
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
    Chest,
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
    pub chest: Option<String>,
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
}

fn default_protocol_rev() -> u32 {
    PROTOCOL_REV
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
}
