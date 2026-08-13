//! Zone 3 (Thornpeak Heights) quest definitions.

use crate::quests::{QuestDef, QuestObjective, QuestRepeat, QuestReward};

pub static ZONE3_QUESTS: &[QuestDef] = &[
    QuestDef {
        id: "stalkers_on_the_ridge",
        name: "Stalkers on the Ridge",
        giver_npc: "commander_elara",
        turn_in_npc: Some("commander_elara"),
        requires: None,
        repeat: QuestRepeat::Once,
        blurb: "Cull the ridge stalkers prowling the western approach to Highwatch.",
        objectives: &[QuestObjective::Kill {
            mob_id: "ridge_stalker",
            count: 4,
            label: "Ridge Stalkers slain",
        }],
        reward: QuestReward {
            xp: 300,
            copper: 90,
            item_id: Some("travelers_ration"),
            choices: &[],
        },
    },
    QuestDef {
        id: "tusks_for_highwatch",
        name: "Tusks for Highwatch",
        giver_npc: "pathfinder_toren",
        turn_in_npc: Some("pathfinder_toren"),
        requires: None,
        repeat: QuestRepeat::Once,
        blurb: "Gather cragback tusks to reinforce the ice hooks along the ascent.",
        objectives: &[QuestObjective::Collect {
            item_id: "boar_tusk",
            count: 3,
            label: "Cragback Tusks",
        }],
        reward: QuestReward {
            xp: 280,
            copper: 85,
            item_id: Some("deepfen_draught"),
            choices: &[],
        },
    },
    QuestDef {
        id: "harpies_over_highwatch",
        name: "Harpies over Highwatch",
        giver_npc: "commander_elara",
        turn_in_npc: Some("commander_elara"),
        requires: Some("stalkers_on_the_ridge"),
        repeat: QuestRepeat::Once,
        blurb: "Drive the gale harpies from the upper crags before they descend on the watchfires.",
        objectives: &[QuestObjective::Kill {
            mob_id: "gale_harpy",
            count: 3,
            label: "Gale Harpies slain",
        }],
        reward: QuestReward {
            xp: 360,
            copper: 110,
            item_id: Some("veteran_helm"),
            choices: &[],
        },
    },
];
