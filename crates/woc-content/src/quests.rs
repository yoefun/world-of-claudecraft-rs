//! Quest definitions for Eastbrook framework slice.

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
}

#[derive(Debug, Clone)]
pub struct QuestReward {
    pub xp: u32,
    pub copper: u32,
    pub item_id: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct QuestDef {
    pub id: &'static str,
    pub name: &'static str,
    pub giver_npc: &'static str,
    pub turn_in_npc: Option<&'static str>,
    pub blurb: &'static str,
    pub objectives: &'static [QuestObjective],
    pub reward: QuestReward,
}

pub static QUESTS: &[QuestDef] = &[
    QuestDef {
        id: "wolves_at_the_gate",
        name: "Wolves at the Gate",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
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
        },
    },
    QuestDef {
        id: "boar_tusks",
        name: "Boar Tusks",
        giver_npc: "captain_alden",
        turn_in_npc: Some("captain_alden"),
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
        },
    },
    QuestDef {
        id: "report_to_alden",
        name: "Report to Alden",
        giver_npc: "town_crier",
        turn_in_npc: Some("captain_alden"),
        blurb: "Speak with Captain Alden in the square.",
        objectives: &[QuestObjective::Talk {
            npc_id: "captain_alden",
            label: "Speak with Captain Alden",
        }],
        reward: QuestReward {
            xp: 20,
            copper: 5,
            item_id: Some("baked_bread"),
        },
    },
];

pub fn quest(id: &str) -> Option<&'static QuestDef> {
    QUESTS.iter().find(|q| q.id == id)
}
