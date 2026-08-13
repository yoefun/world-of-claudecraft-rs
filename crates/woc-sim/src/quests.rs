//! Quest accept, credit, and turn-in.

use crate::ecs::components::QuestProgress;
use crate::ecs::components::{ClassKit, Health, Progress, QuestLog, QuestState};
use crate::ecs::World;
use crate::inventory::{grant_item, player_item_count, take_item};
use crate::types::{player_hp, xp_to_next};
use woc_content::{quest, QuestObjective, QUESTS};
use woc_protocol::{EntityId, QuestLogEntry, SimEvent};

pub struct NpcQuestOffers {
    pub accept: Vec<&'static woc_content::QuestDef>,
    pub turn_in: Vec<&'static woc_content::QuestDef>,
}

pub fn turn_in_npc_id(def: &woc_content::QuestDef) -> &'static str {
    def.turn_in_npc.unwrap_or(def.giver_npc)
}

fn log_state<'a>(log: &'a [QuestLogEntry], quest_id: &str) -> Option<&'a str> {
    log.iter()
        .find(|e| e.quest_id == quest_id)
        .map(|e| e.state.as_str())
}

fn requires_met(def: &woc_content::QuestDef, log: &[QuestLogEntry]) -> bool {
    match def.requires {
        None => true,
        Some(id) => log_state(log, id).is_some_and(|s| s.eq_ignore_ascii_case("completed")),
    }
}

pub fn npc_quest_offers(npc_template_id: &str, log: &[QuestLogEntry]) -> NpcQuestOffers {
    let mut accept = Vec::new();
    let mut turn_in = Vec::new();
    for def in QUESTS.iter() {
        if def.giver_npc == npc_template_id
            && log_state(log, def.id).is_none()
            && requires_met(def, log)
        {
            accept.push(def);
        }
        if turn_in_npc_id(def) == npc_template_id
            && log_state(log, def.id).is_some_and(|s| s.eq_ignore_ascii_case("ready"))
        {
            turn_in.push(def);
        }
    }
    NpcQuestOffers { accept, turn_in }
}

pub fn quest_log_entries(world: &World, player_id: EntityId) -> Vec<QuestLogEntry> {
    world
        .get::<QuestLog>(player_id)
        .map(|q| {
            q.quest_log
                .iter()
                .map(|quest| QuestLogEntry {
                    quest_id: quest.quest_id.clone(),
                    state: match quest.state {
                        QuestState::Active => "active",
                        QuestState::Ready => "ready",
                        QuestState::Completed => "completed",
                    }
                    .to_string(),
                    counts: quest.counts.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn accept_quest(
    world: &mut World,
    player_id: EntityId,
    quest_id: &str,
    giver_template_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = quest(quest_id) else {
        return false;
    };
    if giver_template_id != def.giver_npc {
        events.push(SimEvent::Toast {
            message: "That NPC does not offer this quest.".into(),
        });
        return false;
    }
    let Some(_log) = world.get::<QuestLog>(player_id) else {
        return false;
    };
    let log_entries = quest_log_entries(world, player_id);
    if let Some(state) = log_state(&log_entries, quest_id) {
        let msg = if state.eq_ignore_ascii_case("completed") {
            "You have already completed this quest."
        } else {
            "You have already accepted this quest."
        };
        events.push(SimEvent::Toast {
            message: msg.into(),
        });
        return false;
    }
    if !requires_met(def, &log_entries) {
        events.push(SimEvent::Toast {
            message: "You do not meet the requirements.".into(),
        });
        return false;
    }
    let counts = vec![0u32; def.objectives.len()];
    if let Some(log) = world.get_mut::<QuestLog>(player_id) {
        log.quest_log.push(QuestProgress {
            quest_id: quest_id.to_string(),
            state: QuestState::Active,
            counts,
        });
    }
    events.push(SimEvent::QuestAccepted {
        player: player_id,
        quest_id: quest_id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Accepted: {}", def.name),
    });
    recompute_ready(world, player_id, events);
    true
}

pub fn on_mob_killed(
    world: &mut World,
    player_id: EntityId,
    mob_template_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let updates: Vec<(String, usize, u32, u32, String)> = world
        .get::<QuestLog>(player_id)
        .map(|log| {
            log.quest_log
                .iter()
                .filter(|qp| qp.state == QuestState::Active)
                .flat_map(|qp| {
                    let Some(def) = quest(&qp.quest_id) else {
                        return Vec::new();
                    };
                    def.objectives
                        .iter()
                        .enumerate()
                        .filter_map(|(i, obj)| {
                            if let QuestObjective::Kill {
                                mob_id,
                                count,
                                label,
                            } = obj
                            {
                                if *mob_id != mob_template_id {
                                    return None;
                                }
                                if qp.counts.get(i).copied().unwrap_or(0) >= *count {
                                    return None;
                                }
                                Some((
                                    qp.quest_id.clone(),
                                    i,
                                    qp.counts.get(i).copied().unwrap_or(0) + 1,
                                    *count,
                                    (*label).to_string(),
                                ))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();

    for (quest_id, i, current, required, label) in updates {
        if let Some(log) = world.get_mut::<QuestLog>(player_id) {
            if let Some(qp) = log.quest_log.iter_mut().find(|q| q.quest_id == quest_id) {
                if qp.state == QuestState::Active {
                    qp.counts[i] = current;
                    events.push(SimEvent::QuestProgress {
                        player: player_id,
                        quest_id: qp.quest_id.clone(),
                        objective_index: i as u32,
                        current,
                        required,
                        text: format!("{label}: {current}/{required}"),
                    });
                }
            }
        }
    }
    recompute_ready(world, player_id, events);
}

pub fn on_inventory_changed(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let collect_targets: Vec<(String, usize, String, u32, String)> = world
        .get::<QuestLog>(player_id)
        .map(|log| {
            log.quest_log
                .iter()
                .filter(|qp| qp.state == QuestState::Active)
                .flat_map(|qp| {
                    let Some(def) = quest(&qp.quest_id) else {
                        return Vec::new();
                    };
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
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();

    for (quest_id, i, item_id, count, label) in collect_targets {
        let have = player_item_count(world, player_id, &item_id);
        let new_count = have.min(count);
        if let Some(log) = world.get_mut::<QuestLog>(player_id) {
            let Some(qp) = log.quest_log.iter_mut().find(|q| q.quest_id == quest_id) else {
                continue;
            };
            if qp.state != QuestState::Active {
                continue;
            }
            if new_count != qp.counts[i] {
                qp.counts[i] = new_count;
                events.push(SimEvent::QuestProgress {
                    player: player_id,
                    quest_id: qp.quest_id.clone(),
                    objective_index: i as u32,
                    current: qp.counts[i],
                    required: count,
                    text: format!("{label}: {}/{count}", qp.counts[i]),
                });
            }
        }
    }
    recompute_ready(world, player_id, events);
}

pub fn on_talked_to(
    world: &mut World,
    player_id: EntityId,
    npc_template_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let updates: Vec<(String, usize, String)> = world
        .get::<QuestLog>(player_id)
        .map(|log| {
            log.quest_log
                .iter()
                .filter(|qp| qp.state == QuestState::Active)
                .flat_map(|qp| {
                    let Some(def) = quest(&qp.quest_id) else {
                        return Vec::new();
                    };
                    def.objectives
                        .iter()
                        .enumerate()
                        .filter_map(|(i, obj)| {
                            if let QuestObjective::Talk { npc_id, label } = obj {
                                if *npc_id != npc_template_id
                                    || qp.counts.get(i).copied().unwrap_or(0) >= 1
                                {
                                    return None;
                                }
                                Some((qp.quest_id.clone(), i, (*label).to_string()))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();

    for (quest_id, i, label) in updates {
        if let Some(log) = world.get_mut::<QuestLog>(player_id) {
            if let Some(qp) = log.quest_log.iter_mut().find(|q| q.quest_id == quest_id) {
                if qp.state == QuestState::Active {
                    qp.counts[i] = 1;
                    events.push(SimEvent::QuestProgress {
                        player: player_id,
                        quest_id: qp.quest_id.clone(),
                        objective_index: i as u32,
                        current: 1,
                        required: 1,
                        text: format!("{label}: 1/1"),
                    });
                }
            }
        }
    }
    recompute_ready(world, player_id, events);
}

pub fn recompute_ready(world: &mut World, player_id: EntityId, _events: &mut Vec<SimEvent>) {
    let Some(log) = world.get_mut::<QuestLog>(player_id) else {
        return;
    };
    for qp in log.quest_log.iter_mut() {
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

pub fn grant_xp_world(
    world: &mut World,
    player_id: EntityId,
    amount: u32,
    events: &mut Vec<SimEvent>,
) {
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.xp = p.xp.saturating_add(amount);
    }
    loop {
        let level = world.get::<Health>(player_id).map(|h| h.level).unwrap_or(1);
        let xp = world.get::<Progress>(player_id).map(|p| p.xp).unwrap_or(0);
        let need = xp_to_next(level);
        if xp < need {
            break;
        }
        if let Some(p) = world.get_mut::<Progress>(player_id) {
            p.xp -= need;
        }
        let class = world.get::<ClassKit>(player_id).and_then(|k| k.class_id);
        let armor = world
            .get::<crate::ecs::components::Combat>(player_id)
            .map(|c| c.armor)
            .unwrap_or(0.0);
        let new_level = world
            .get::<Health>(player_id)
            .map(|h| h.level + 1)
            .unwrap_or(1);
        if let Some(h) = world.get_mut::<Health>(player_id) {
            h.level = new_level;
            if let Some(class) = class {
                let def = woc_content::class_def(class);
                h.hp_max = player_hp(def.base_hp, h.level) + armor * 0.5;
            }
            h.hp = h.hp_max;
            events.push(SimEvent::LevelUp {
                player: player_id,
                level: h.level,
            });
            events.push(SimEvent::Toast {
                message: format!("You reached level {}!", h.level),
            });
        }
        if let (Some(class), Some(kit)) = (class, world.get_mut::<ClassKit>(player_id)) {
            kit.known_abilities = woc_content::known_abilities_at_level(class, new_level)
                .into_iter()
                .map(|s| s.to_string())
                .collect();
        }
        crate::talents::on_level_up(world, player_id);
    }
}

pub fn turn_in_quest(
    world: &mut World,
    player_id: EntityId,
    quest_id: &str,
    turner_template_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let ready = world.get::<QuestLog>(player_id).and_then(|log| {
        log.quest_log
            .iter()
            .position(|q| q.quest_id == quest_id && q.state == QuestState::Ready)
    });
    let Some(idx) = ready else {
        events.push(SimEvent::Toast {
            message: "Nothing to turn in.".into(),
        });
        return false;
    };
    let Some(def) = quest(quest_id) else {
        return false;
    };
    if turner_template_id != turn_in_npc_id(def) {
        events.push(SimEvent::Toast {
            message: "This NPC cannot take this quest.".into(),
        });
        return false;
    }

    for obj in def.objectives {
        if let QuestObjective::Collect { item_id, count, .. } = obj {
            if take_item(world, player_id, item_id, *count, events).is_err() {
                return false;
            }
        }
    }

    if let Some(log) = world.get_mut::<QuestLog>(player_id) {
        log.quest_log[idx].state = QuestState::Completed;
    }
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_add(def.reward.copper);
    }
    grant_xp_world(world, player_id, def.reward.xp, events);
    if let Some(item_id) = def.reward.item_id {
        let _ = grant_item(world, player_id, item_id, 1, events);
    }
    events.push(SimEvent::QuestCompleted {
        player: player_id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::{Identity, Transform};
    use crate::sim::Sim;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, QuestLogEntry};

    fn find_template(sim: &Sim, template: &str) -> EntityId {
        sim.world
            .live_ids()
            .find(|&id| {
                sim.world
                    .get::<Identity>(id)
                    .and_then(|i| i.template_id.as_deref())
                    == Some(template)
            })
            .expect(template)
    }

    fn place_at_template(sim: &mut Sim, template: &str) {
        let id = find_template(sim, template);
        let (x, z) = {
            let t = sim.world.get::<Transform>(id).unwrap();
            (t.x, t.z)
        };
        let y = crate::ecs::spawn::ground_at(x, z);
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = x;
            t.z = z;
            t.y = y;
        }
    }

    fn log_entries(sim: &Sim) -> Vec<QuestLogEntry> {
        sim.snapshot_for_player(sim.player_id).quest_log
    }

    #[test]
    fn offers_hide_locked_and_completed() {
        let empty: Vec<QuestLogEntry> = vec![];
        let crier = npc_quest_offers("town_crier", &empty);
        assert!(crier.accept.iter().any(|q| q.id == "report_to_alden"));
        assert!(crier.turn_in.is_empty());

        let alden = npc_quest_offers("captain_alden", &empty);
        assert!(
            !alden.accept.iter().any(|q| q.id == "wolves_at_the_gate"),
            "wolves locked until report_to_alden is completed"
        );

        let after_report = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        }];
        let alden = npc_quest_offers("captain_alden", &after_report);
        assert!(alden.accept.iter().any(|q| q.id == "wolves_at_the_gate"));
    }

    #[test]
    fn accept_rejects_wrong_npc_and_missing_prereq() {
        let mut sim = Sim::new_eastbrook("Qgate", PlayerClass::Warrior);
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );
        assert!(log_entries(&sim).is_empty());

        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            },
        );
        assert!(log_entries(&sim).is_empty());
    }
}
