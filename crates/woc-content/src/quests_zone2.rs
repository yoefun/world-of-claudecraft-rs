//! Zone 2 (Eastfen Marsh) quest definitions.

use crate::quests::{QuestDef, QuestObjective, QuestReward};

pub static ZONE2_QUESTS: &[QuestDef] = &[
    QuestDef {
        id: "report_to_selene",
        name: "Report to Selene",
        giver_npc: "scout_darian",
        turn_in_npc: Some("warden_selene"),
        blurb: "Find Warden Selene at the Eastfen boardwalk outpost.",
        objectives: &[QuestObjective::Talk {
            npc_id: "warden_selene",
            label: "Speak with Warden Selene",
        }],
        reward: QuestReward {
            xp: 35,
            copper: 10,
            item_id: Some("fen_tonic"),
        },
    },
    QuestDef {
        id: "crawler_cull",
        name: "Crawler Cull",
        giver_npc: "warden_selene",
        turn_in_npc: Some("warden_selene"),
        blurb: "Thin the fen crawlers nesting west of the boardwalk.",
        objectives: &[QuestObjective::Kill {
            mob_id: "fen_crawler",
            count: 4,
            label: "Fen Crawlers slain",
        }],
        reward: QuestReward {
            xp: 120,
            copper: 40,
            item_id: Some("reedwalk_boots"),
        },
    },
    QuestDef {
        id: "toad_bile_harvest",
        name: "Toad Bile Harvest",
        giver_npc: "apothecary_vex",
        turn_in_npc: Some("apothecary_vex"),
        blurb: "Gather bile from mire toads along the south pools.",
        objectives: &[QuestObjective::Collect {
            item_id: "toad_bile",
            count: 3,
            label: "Toad Bile",
        }],
        reward: QuestReward {
            xp: 100,
            copper: 35,
            item_id: Some("fen_tonic"),
        },
    },
    QuestDef {
        id: "wisps_in_the_mist",
        name: "Wisps in the Mist",
        giver_npc: "warden_selene",
        turn_in_npc: Some("warden_selene"),
        blurb: "Drive off bog wisps haunting the northeast reeds.",
        objectives: &[QuestObjective::Kill {
            mob_id: "bog_wisp",
            count: 3,
            label: "Bog Wisps slain",
        }],
        reward: QuestReward {
            xp: 140,
            copper: 50,
            item_id: Some("marsh_wraps"),
        },
    },
    QuestDef {
        id: "silk_for_bandages",
        name: "Silk for Bandages",
        giver_npc: "apothecary_vex",
        turn_in_npc: Some("apothecary_vex"),
        blurb: "Collect fen silk from crawlers for marsh dressings.",
        objectives: &[QuestObjective::Collect {
            item_id: "fen_silk",
            count: 4,
            label: "Fen Silk",
        }],
        reward: QuestReward {
            xp: 110,
            copper: 30,
            item_id: None,
        },
    },
    QuestDef {
        id: "ember_offering",
        name: "Ember Offering",
        giver_npc: "scout_darian",
        turn_in_npc: Some("apothecary_vex"),
        blurb: "Recover wisp embers and deliver them to Apothecary Vex.",
        objectives: &[QuestObjective::Collect {
            item_id: "wisp_ember",
            count: 2,
            label: "Wisp Embers",
        }],
        reward: QuestReward {
            xp: 125,
            copper: 45,
            item_id: Some("fen_tonic"),
        },
    },
];
