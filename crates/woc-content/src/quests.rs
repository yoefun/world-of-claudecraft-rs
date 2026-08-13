//! Quest definitions for Eastbrook framework slice.

use std::sync::LazyLock;

use crate::quests_zone2::ZONE2_QUESTS;
use crate::quests_zone3::ZONE3_QUESTS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestRepeat {
    Once,
    Daily,
}

#[derive(Debug, Clone)]
pub enum QuestObjective {
    Kill {
        mob_id: &'static str,
        count: u32,
        label: &'static str,
    },
    Collect {
        item_id: &'static str,
        count: u32,
        label: &'static str,
    },
    Talk {
        npc_id: &'static str,
        label: &'static str,
    },
    Explore {
        x: f32,
        z: f32,
        radius: f32,
        label: &'static str,
    },
    Escort {
        npc_id: &'static str,
        dest_x: f32,
        dest_z: f32,
        radius: f32,
        label: &'static str,
    },
}

#[derive(Debug, Clone)]
pub struct QuestReward {
    pub xp: u32,
    pub copper: u32,
    pub item_id: Option<&'static str>,
    pub choices: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct QuestDef {
    pub id: &'static str,
    pub name: &'static str,
    pub giver_npc: &'static str,
    pub turn_in_npc: Option<&'static str>,
    pub requires: Option<&'static str>,
    pub repeat: QuestRepeat,
    pub blurb: &'static str,
    pub objectives: &'static [QuestObjective],
    pub reward: QuestReward,
}

/// Ten minutes of sim time at 20 Hz. Daily epoch = `tick / DAILY_PERIOD_TICKS`.
pub const DAILY_PERIOD_TICKS: u64 = 12_000;

pub static ZONE1_QUESTS: &[QuestDef] = &[
    QuestDef {
        id: "wolves_at_the_gate",
        name: "Wolves at the Gate",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
        requires: Some("report_to_alden"),
        repeat: QuestRepeat::Once,
        blurb: "Slay young wolves north of town.",
        objectives: &[QuestObjective::Kill {
            mob_id: "young_wolf",
            count: 3,
            label: "Young Wolves slain",
        }],
        reward: QuestReward {
            xp: 80,
            copper: 25,
            item_id: Some("eastbrook_greaves"),
            choices: &[],
        },
    },
    QuestDef {
        id: "boar_tusks",
        name: "Boar Tusks",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
        requires: Some("wolves_at_the_gate"),
        repeat: QuestRepeat::Once,
        blurb: "Collect tusks from the eastern meadow.",
        objectives: &[QuestObjective::Collect {
            item_id: "boar_tusk",
            count: 2,
            label: "Boar Tusks",
        }],
        reward: QuestReward {
            xp: 60,
            copper: 20,
            item_id: None,
            choices: &[],
        },
    },
    QuestDef {
        id: "report_to_alden",
        name: "Report to Alden",
        giver_npc: "town_crier",
        turn_in_npc: Some("captain_alden"),
        requires: None,
        repeat: QuestRepeat::Once,
        blurb: "Speak with Captain Alden in the square.",
        objectives: &[QuestObjective::Talk {
            npc_id: "captain_alden",
            label: "Speak with Captain Alden",
        }],
        reward: QuestReward {
            xp: 20,
            copper: 5,
            item_id: Some("baked_bread"),
            choices: &[],
        },
    },
    QuestDef {
        id: "scout_north_road",
        name: "Scout the North Road",
        giver_npc: "town_crier",
        turn_in_npc: Some("town_crier"),
        requires: Some("report_to_alden"),
        repeat: QuestRepeat::Once,
        blurb: "Walk the north road toward Wolf Run and report what you see.",
        objectives: &[QuestObjective::Explore {
            x: -8.0,
            z: 40.0,
            radius: 12.0,
            label: "North road scouted",
        }],
        reward: QuestReward {
            xp: 25,
            copper: 8,
            item_id: None,
            choices: &[],
        },
    },
    QuestDef {
        id: "wolf_patrol",
        name: "Wolf Patrol",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
        requires: Some("wolves_at_the_gate"),
        repeat: QuestRepeat::Daily,
        blurb: "Thin the pack again before nightfall.",
        objectives: &[QuestObjective::Kill {
            mob_id: "young_wolf",
            count: 2,
            label: "Young Wolves slain",
        }],
        reward: QuestReward {
            xp: 40,
            copper: 15,
            item_id: None,
            choices: &[],
        },
    },
    QuestDef {
        id: "courier_to_the_gate",
        name: "Courier to the Gate",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
        requires: Some("boar_tusks"),
        repeat: QuestRepeat::Once,
        blurb: "Escort the Eastbrook courier up the north road to the wolf gate.",
        objectives: &[QuestObjective::Escort {
            npc_id: "eastbrook_courier",
            dest_x: -8.0,
            dest_z: 50.0,
            radius: 8.0,
            label: "Courier reached the gate",
        }],
        reward: QuestReward {
            xp: 50,
            copper: 20,
            item_id: None,
            choices: &[],
        },
    },
    QuestDef {
        id: "arms_of_the_watch",
        name: "Arms of the Watch",
        giver_npc: "trader_wilkes",
        turn_in_npc: Some("trader_wilkes"),
        requires: Some("report_to_alden"),
        repeat: QuestRepeat::Once,
        blurb: "Speak with Captain Alden, then choose a ration from Wilkes' stores.",
        objectives: &[QuestObjective::Talk {
            npc_id: "captain_alden",
            label: "Speak with Captain Alden",
        }],
        reward: QuestReward {
            xp: 30,
            copper: 10,
            item_id: None,
            choices: &["travelers_ration", "spring_water", "baked_bread"],
        },
    },
];

/// Zone1 + zone2 + zone3 quest definitions.
pub static QUESTS: LazyLock<&'static [QuestDef]> = LazyLock::new(|| {
    let mut all = Vec::with_capacity(ZONE1_QUESTS.len() + ZONE2_QUESTS.len() + ZONE3_QUESTS.len());
    all.extend_from_slice(ZONE1_QUESTS);
    all.extend_from_slice(ZONE2_QUESTS);
    all.extend_from_slice(ZONE3_QUESTS);
    Box::leak(all.into_boxed_slice())
});

pub fn quest(id: &str) -> Option<&'static QuestDef> {
    QUESTS.iter().find(|q| q.id == id)
}
