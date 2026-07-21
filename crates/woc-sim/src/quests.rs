//! Quest accept, credit, and turn-in.

use crate::entity::Entity;
use crate::entity::{QuestProgress, QuestState};
use crate::inventory::{grant_item, player_item_count, take_item};
use woc_content::{quest, QuestObjective, QUESTS};
use woc_protocol::SimEvent;

pub fn accept_quest(player: &mut Entity, quest_id: &str, events: &mut Vec<SimEvent>) -> bool {
    let Some(def) = quest(quest_id) else {
        return false;
    };
    if player
        .quest_log
        .iter()
        .any(|q| q.quest_id == quest_id && q.state != QuestState::Completed)
    {
        return false;
    }
    let counts = vec![0u32; def.objectives.len()];
    player.quest_log.push(QuestProgress {
        quest_id: quest_id.to_string(),
        state: QuestState::Active,
        counts,
    });
    events.push(SimEvent::QuestAccepted {
        player: player.id,
        quest_id: quest_id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Accepted: {}", def.name),
    });
    recompute_ready(player, events);
    true
}

pub fn on_mob_killed(player: &mut Entity, mob_template_id: &str, events: &mut Vec<SimEvent>) {
    for qp in player.quest_log.iter_mut() {
        if qp.state != QuestState::Active {
            continue;
        }
        let Some(def) = quest(&qp.quest_id) else {
            continue;
        };
        for (i, obj) in def.objectives.iter().enumerate() {
            if let QuestObjective::Kill {
                mob_id,
                count,
                label,
            } = obj
            {
                if *mob_id != mob_template_id {
                    continue;
                }
                if qp.counts[i] >= *count {
                    continue;
                }
                qp.counts[i] += 1;
                events.push(SimEvent::QuestProgress {
                    player: player.id,
                    quest_id: qp.quest_id.clone(),
                    objective_index: i as u32,
                    current: qp.counts[i],
                    required: *count,
                    text: format!("{label}: {}/{count}", qp.counts[i]),
                });
            }
        }
    }
    recompute_ready(player, events);
}

pub fn on_inventory_changed(player: &mut Entity, events: &mut Vec<SimEvent>) {
    // Snapshot collect counts before mutating quest_log (avoid borrow clash).
    let collect_targets: Vec<(String, usize, String, u32, String)> = player
        .quest_log
        .iter()
        .filter(|qp| qp.state == QuestState::Active)
        .filter_map(|qp| {
            let def = quest(&qp.quest_id)?;
            Some(
                def.objectives
                    .iter()
                    .enumerate()
                    .filter_map(|(i, obj)| {
                        if let QuestObjective::Collect {
                            item_id,
                            count,
                            label,
                        } = obj
                        {
                            Some((
                                qp.quest_id.clone(),
                                i,
                                (*item_id).to_string(),
                                *count,
                                (*label).to_string(),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    for (quest_id, i, item_id, count, label) in collect_targets {
        let have = player_item_count(player, &item_id);
        let new_count = have.min(count);
        let Some(qp) = player.quest_log.iter_mut().find(|q| q.quest_id == quest_id) else {
            continue;
        };
        if qp.state != QuestState::Active {
            continue;
        }
        if new_count != qp.counts[i] {
            qp.counts[i] = new_count;
            events.push(SimEvent::QuestProgress {
                player: player.id,
                quest_id: qp.quest_id.clone(),
                objective_index: i as u32,
                current: qp.counts[i],
                required: count,
                text: format!("{label}: {}/{count}", qp.counts[i]),
            });
        }
    }
    recompute_ready(player, events);
}

pub fn on_talked_to(player: &mut Entity, npc_template_id: &str, events: &mut Vec<SimEvent>) {
    for qp in player.quest_log.iter_mut() {
        if qp.state != QuestState::Active {
            continue;
        }
        let Some(def) = quest(&qp.quest_id) else {
            continue;
        };
        for (i, obj) in def.objectives.iter().enumerate() {
            if let QuestObjective::Talk { npc_id, label } = obj {
                if *npc_id != npc_template_id || qp.counts[i] >= 1 {
                    continue;
                }
                qp.counts[i] = 1;
                events.push(SimEvent::QuestProgress {
                    player: player.id,
                    quest_id: qp.quest_id.clone(),
                    objective_index: i as u32,
                    current: 1,
                    required: 1,
                    text: format!("{label}: 1/1"),
                });
            }
        }
    }
    recompute_ready(player, events);
}

pub fn recompute_ready(player: &mut Entity, _events: &mut Vec<SimEvent>) {
    for qp in player.quest_log.iter_mut() {
        if qp.state == QuestState::Completed {
            continue;
        }
        let Some(def) = quest(&qp.quest_id) else {
            continue;
        };
        let done = def.objectives.iter().enumerate().all(|(i, obj)| {
            let need = match obj {
                QuestObjective::Kill { count, .. } => *count,
                QuestObjective::Collect { count, .. } => *count,
                QuestObjective::Talk { .. } => 1,
            };
            qp.counts.get(i).copied().unwrap_or(0) >= need
        });
        qp.state = if done {
            QuestState::Ready
        } else {
            QuestState::Active
        };
    }
}

pub fn turn_in_quest(player: &mut Entity, quest_id: &str, events: &mut Vec<SimEvent>) -> bool {
    let Some(idx) = player
        .quest_log
        .iter()
        .position(|q| q.quest_id == quest_id && q.state == QuestState::Ready)
    else {
        return false;
    };
    let Some(def) = quest(quest_id) else {
        return false;
    };

    // Consume collect items.
    for obj in def.objectives {
        if let QuestObjective::Collect { item_id, count, .. } = obj {
            if take_item(player, item_id, *count, events).is_err() {
                return false;
            }
        }
    }

    player.quest_log[idx].state = QuestState::Completed;
    player.copper = player.copper.saturating_add(def.reward.copper);
    crate::combat::grant_xp(player, def.reward.xp, events);
    if let Some(item_id) = def.reward.item_id {
        let _ = grant_item(player, item_id, 1, events);
    }
    events.push(SimEvent::QuestCompleted {
        player: player.id,
        quest_id: quest_id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Completed: {}", def.name),
    });
    true
}

pub fn quests_for_npc(npc_template_id: &str) -> Vec<&'static woc_content::QuestDef> {
    QUESTS
        .iter()
        .filter(|q| q.giver_npc == npc_template_id || q.turn_in_npc == Some(npc_template_id))
        .collect()
}
