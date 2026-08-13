//! Zone 2 (Eastfen Marsh and Mirefen) quest definitions.

use crate::quests::{QuestDef, QuestObjective, QuestReward};

pub static ZONE2_QUESTS: &[QuestDef] = &[
    QuestDef {
        id: "report_to_selene",
        name: "Report to Selene",
        giver_npc: "scout_darian",
        turn_in_npc: Some("warden_selene"),
        requires: None,
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
        requires: Some("report_to_selene"),
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
        requires: None,
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
        requires: Some("crawler_cull"),
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
        requires: None,
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
        requires: None,
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
    QuestDef {
        id: "into_mirefen",
        name: "Into Mirefen",
        giver_npc: "warden_selene",
        turn_in_npc: Some("keeper_orla"),
        requires: Some("wisps_in_the_mist"),
        blurb: "Carry Selene's warning to Keeper Orla at the Mirefen lantern camp.",
        objectives: &[QuestObjective::Talk {
            npc_id: "keeper_orla",
            label: "Speak with Keeper Orla",
        }],
        reward: QuestReward {
            xp: 90,
            copper: 30,
            item_id: Some("deepfen_draught"),
        },
    },
    QuestDef {
        id: "leeches_at_the_landing",
        name: "Leeches at the Landing",
        giver_npc: "keeper_orla",
        turn_in_npc: Some("keeper_orla"),
        requires: Some("into_mirefen"),
        blurb: "Clear the mire leeches clustering around the western landing.",
        objectives: &[QuestObjective::Kill {
            mob_id: "mire_leech",
            count: 4,
            label: "Mire Leeches slain",
        }],
        reward: QuestReward {
            xp: 190,
            copper: 65,
            item_id: Some("mireguard_hood"),
        },
    },
    QuestDef {
        id: "spores_for_the_ferryman",
        name: "Spores for the Ferryman",
        giver_npc: "ferryman_noll",
        turn_in_npc: Some("ferryman_noll"),
        requires: None,
        blurb: "Gather rotcap spores to keep Noll's signal brazier burning through the fog.",
        objectives: &[QuestObjective::Collect {
            item_id: "rotcap_spore",
            count: 3,
            label: "Rotcap Spores",
        }],
        reward: QuestReward {
            xp: 210,
            copper: 70,
            item_id: Some("deepfen_draught"),
        },
    },
    QuestDef {
        id: "terror_beneath_the_reeds",
        name: "Terror Beneath the Reeds",
        giver_npc: "keeper_orla",
        turn_in_npc: Some("keeper_orla"),
        requires: Some("leeches_at_the_landing"),
        blurb: "Slay the Mire Terror in the eastern sinkhole before it reaches the lantern camp.",
        objectives: &[QuestObjective::Kill {
            mob_id: "mire_terror",
            count: 1,
            label: "Mire Terror slain",
        }],
        reward: QuestReward {
            xp: 500,
            copper: 150,
            item_id: Some("deepfen_draught"),
        },
    },
];
