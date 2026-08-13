//! Quest accept, credit, and turn-in.

use crate::ecs::components::QuestProgress;
use crate::ecs::components::{
    ClassKit, Escort, Health, Identity, Progress, QuestLog, QuestState, Transform,
};
use crate::ecs::spawn::create_npc_from_template;
use crate::ecs::World;
use crate::entity_motion::step_toward;
use crate::inventory::{grant_item, player_item_count, take_item};
use crate::social::party::PartyRoster;
use crate::types::{player_hp, xp_to_next, MOB_SPEED};
use woc_content::{quest, QuestObjective, QuestRepeat, DAILY_PERIOD_TICKS, QUESTS};
use woc_protocol::{EntityId, QuestLogEntry, SimEvent};

pub struct NpcQuestOffers {
    pub accept: Vec<&'static woc_content::QuestDef>,
    pub turn_in: Vec<&'static woc_content::QuestDef>,
}

pub fn turn_in_npc_id(def: &woc_content::QuestDef) -> &'static str {
    def.turn_in_npc.unwrap_or(def.giver_npc)
}

fn objective_required(obj: &QuestObjective) -> u32 {
    match obj {
        QuestObjective::Kill { count, .. } | QuestObjective::Collect { count, .. } => *count,
        QuestObjective::Talk { .. }
        | QuestObjective::Explore { .. }
        | QuestObjective::Escort { .. } => 1,
    }
}

fn daily_epoch(tick: u64) -> u64 {
    tick / DAILY_PERIOD_TICKS
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
            completed_tick: 0,
        });
    }
    events.push(SimEvent::QuestAccepted {
        player: player_id,
        quest_id: quest_id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Accepted: {}", def.name),
    });
    spawn_escort_for(world, player_id, def, events);
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

pub fn recompute_ready(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let Some(log) = world.get_mut::<QuestLog>(player_id) else {
        return;
    };
    let mut became_ready: Vec<String> = Vec::new();
    for qp in log.quest_log.iter_mut() {
        if qp.state == QuestState::Completed {
            continue;
        }
        let Some(def) = quest(&qp.quest_id) else {
            continue;
        };
        let done = def
            .objectives
            .iter()
            .enumerate()
            .all(|(i, obj)| qp.counts.get(i).copied().unwrap_or(0) >= objective_required(obj));
        let next = if done {
            QuestState::Ready
        } else {
            QuestState::Active
        };
        if qp.state != QuestState::Ready && next == QuestState::Ready {
            became_ready.push(def.name.to_string());
        }
        qp.state = next;
    }
    for name in became_ready {
        events.push(SimEvent::Toast {
            message: format!("Ready to turn in: {name}"),
        });
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
    now_tick: u64,
    reward_choice: Option<u32>,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = quest(quest_id) else {
        return false;
    };
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
    if turner_template_id != turn_in_npc_id(def) {
        events.push(SimEvent::Toast {
            message: "This NPC cannot take this quest.".into(),
        });
        return false;
    }
    if !def.reward.choices.is_empty() {
        let Some(choice) = reward_choice else {
            events.push(SimEvent::Toast {
                message: "Choose a reward.".into(),
            });
            return false;
        };
        if def.reward.choices.get(choice as usize).is_none() {
            events.push(SimEvent::Toast {
                message: "Choose a reward.".into(),
            });
            return false;
        }
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
        log.quest_log[idx].completed_tick = now_tick;
    }
    despawn_escorts(world, player_id, quest_id);
    if let Some(p) = world.get_mut::<Progress>(player_id) {
        p.copper = p.copper.saturating_add(def.reward.copper);
    }
    grant_xp_world(world, player_id, def.reward.xp, events);
    if let Some(item_id) = def.reward.item_id {
        let _ = grant_item(world, player_id, item_id, 1, events);
    }
    if let Some(choice) = reward_choice {
        if let Some(item_id) = def.reward.choices.get(choice as usize) {
            let _ = grant_item(world, player_id, item_id, 1, events);
        }
    }
    if let Some(rep) = def.reward.reputation {
        crate::reputation::award(world, player_id, rep.faction_id, rep.amount, events);
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

pub fn abandon_quest(
    world: &mut World,
    player_id: EntityId,
    quest_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = quest(quest_id) else {
        return false;
    };
    let idx = world.get::<QuestLog>(player_id).and_then(|log| {
        log.quest_log
            .iter()
            .position(|q| q.quest_id == quest_id && q.state != QuestState::Completed)
    });
    let Some(idx) = idx else {
        events.push(SimEvent::Toast {
            message: "Nothing to abandon.".into(),
        });
        return false;
    };
    if let Some(log) = world.get_mut::<QuestLog>(player_id) {
        log.quest_log.remove(idx);
    }
    despawn_escorts(world, player_id, quest_id);
    events.push(SimEvent::QuestAbandoned {
        player: player_id,
        quest_id: quest_id.to_string(),
    });
    events.push(SimEvent::Toast {
        message: format!("Abandoned: {}", def.name),
    });
    true
}

pub fn share_quest(
    world: &mut World,
    parties: &PartyRoster,
    player_id: EntityId,
    quest_id: &str,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(def) = quest(quest_id) else {
        return false;
    };
    let sharer_ok = world.get::<QuestLog>(player_id).is_some_and(|log| {
        log.quest_log
            .iter()
            .any(|q| q.quest_id == quest_id && q.state != QuestState::Completed)
    });
    if !sharer_ok {
        events.push(SimEvent::Toast {
            message: "You cannot share that quest.".into(),
        });
        return false;
    }
    let Some(members) = parties.members_of(player_id) else {
        events.push(SimEvent::Toast {
            message: "You are not in a party.".into(),
        });
        return false;
    };
    let in_range = kill_credit_share_ids(parties, world, player_id);
    let mut any = false;
    for mate in members.into_iter().filter(|id| *id != player_id) {
        let mate_name = world
            .get::<Identity>(mate)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "Someone".into());
        if !in_range.contains(&mate) {
            events.push(SimEvent::Toast {
                message: format!("{mate_name} is too far away."),
            });
            continue;
        }
        if accept_quest(world, mate, quest_id, def.giver_npc, events) {
            any = true;
        }
    }
    if any {
        events.push(SimEvent::Toast {
            message: format!("Shared: {}", def.name),
        });
    }
    any
}

fn kill_credit_share_ids(
    parties: &PartyRoster,
    world: &World,
    player_id: EntityId,
) -> Vec<EntityId> {
    crate::social::party::kill_credit_share(parties, world, player_id)
}

pub fn refresh_daily_quests(world: &mut World, player_id: EntityId, now_tick: u64) {
    let Some(log) = world.get_mut::<QuestLog>(player_id) else {
        return;
    };
    log.quest_log.retain(|qp| {
        if qp.state != QuestState::Completed {
            return true;
        }
        let Some(def) = quest(&qp.quest_id) else {
            return true;
        };
        if def.repeat != QuestRepeat::Daily {
            return true;
        }
        daily_epoch(qp.completed_tick) == daily_epoch(now_tick)
    });
}

pub fn credit_explore(world: &mut World, player_id: EntityId, events: &mut Vec<SimEvent>) {
    let Some(pt) = world.get::<Transform>(player_id).copied() else {
        return;
    };
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
                            if qp.counts.get(i).copied().unwrap_or(0) >= 1 {
                                return None;
                            }
                            let QuestObjective::Explore {
                                x,
                                z,
                                radius,
                                label,
                            } = obj
                            else {
                                return None;
                            };
                            let dx = pt.x - *x;
                            let dz = pt.z - *z;
                            if (dx * dx + dz * dz).sqrt() > *radius {
                                return None;
                            }
                            Some((qp.quest_id.clone(), i, (*label).to_string()))
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();
    let any = !updates.is_empty();
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
    if any {
        recompute_ready(world, player_id, events);
    }
}

fn spawn_escort_for(
    world: &mut World,
    player_id: EntityId,
    def: &woc_content::QuestDef,
    events: &mut Vec<SimEvent>,
) {
    for obj in def.objectives {
        let QuestObjective::Escort {
            npc_id,
            dest_x,
            dest_z,
            radius,
            ..
        } = obj
        else {
            continue;
        };
        despawn_escorts(world, player_id, def.id);
        let Some(pt) = world.get::<Transform>(player_id).copied() else {
            continue;
        };
        let id = world.next_id();
        if create_npc_from_template(world, id, npc_id, pt.x, pt.z).is_none() {
            events.push(SimEvent::Toast {
                message: "Escort NPC is missing.".into(),
            });
            continue;
        }
        world.insert(
            id,
            Escort {
                player_id,
                quest_id: def.id.to_string(),
                dest_x: *dest_x,
                dest_z: *dest_z,
                radius: *radius,
            },
        );
        if let Some(h) = world.get_mut::<Health>(id) {
            h.hp = 80.0;
            h.hp_max = 80.0;
        }
    }
}

pub fn despawn_escorts(world: &mut World, player_id: EntityId, quest_id: &str) {
    let drop: Vec<EntityId> = world
        .ids::<Escort>()
        .into_iter()
        .filter(|&id| {
            world
                .get::<Escort>(id)
                .is_some_and(|e| e.player_id == player_id && e.quest_id == quest_id)
        })
        .collect();
    for id in drop {
        world.despawn(id);
    }
}

pub fn tick_escorts(world: &mut World, events: &mut Vec<SimEvent>) {
    let ids = world.ids::<Escort>();
    let mut failed: Vec<(EntityId, EntityId, String, String)> = Vec::new();
    let mut arrived: Vec<(EntityId, String)> = Vec::new();
    for id in ids {
        let Some(esc) = world.get::<Escort>(id).cloned() else {
            continue;
        };
        let alive = world
            .get::<Health>(id)
            .map(|h| h.alive && h.hp > 0.0)
            .unwrap_or(false);
        if !alive {
            let name = quest(&esc.quest_id)
                .map(|d| d.name.to_string())
                .unwrap_or_else(|| esc.quest_id.clone());
            failed.push((id, esc.player_id, esc.quest_id.clone(), name));
            continue;
        }
        if let Some(pt) = world.get::<Transform>(esc.player_id).copied() {
            let here = world.get::<Transform>(id).copied();
            if let Some(ht) = here {
                let dx = pt.x - ht.x;
                let dz = pt.z - ht.z;
                if (dx * dx + dz * dz).sqrt() > 3.0 {
                    step_toward(world, id, pt.x, pt.z, MOB_SPEED);
                }
            }
        }
        if let Some(ht) = world.get::<Transform>(id).copied() {
            let dx = ht.x - esc.dest_x;
            let dz = ht.z - esc.dest_z;
            if (dx * dx + dz * dz).sqrt() <= esc.radius {
                arrived.push((esc.player_id, esc.quest_id.clone()));
            }
        }
    }
    for (npc_id, player_id, quest_id, name) in failed {
        if let Some(log) = world.get_mut::<QuestLog>(player_id) {
            log.quest_log
                .retain(|q| !(q.quest_id == quest_id && q.state != QuestState::Completed));
        }
        world.despawn(npc_id);
        events.push(SimEvent::Toast {
            message: format!("Escort failed: {name}."),
        });
    }
    for (player_id, quest_id) in arrived {
        credit_escort(world, player_id, &quest_id, events);
    }
}

fn credit_escort(
    world: &mut World,
    player_id: EntityId,
    quest_id: &str,
    events: &mut Vec<SimEvent>,
) {
    let Some(def) = quest(quest_id) else {
        return;
    };
    let Some(i) = def
        .objectives
        .iter()
        .position(|o| matches!(o, QuestObjective::Escort { .. }))
    else {
        return;
    };
    let label = match def.objectives[i] {
        QuestObjective::Escort { label, .. } => label,
        _ => return,
    };
    if let Some(log) = world.get_mut::<QuestLog>(player_id) {
        if let Some(qp) = log.quest_log.iter_mut().find(|q| q.quest_id == quest_id) {
            if qp.state == QuestState::Active && qp.counts.get(i).copied().unwrap_or(0) < 1 {
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
    recompute_ready(world, player_id, events);
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
    use crate::ecs::components::{
        Health, Identity, QuestLog, QuestProgress, QuestState, Transform,
    };
    use crate::sim::Sim;
    use woc_content::PlayerClass;
    use woc_protocol::{InteractAction, QuestLogEntry, SimEvent};

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

    #[test]
    fn talk_quest_accept_talk_turnin() {
        let mut sim = Sim::new_eastbrook("Qtalk", PlayerClass::Warrior);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(alden, InteractAction::Talk);
        let ready = log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "ready");
        assert!(ready);
        sim.interact(
            alden,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
                reward_choice: None,
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "completed"));
        assert!(sim
            .snapshot_for_player(sim.player_id)
            .inventory
            .iter()
            .any(|s| s.item_id == "baked_bread"));
    }

    #[test]
    fn collect_quest_grant_ready_turnin_consumes() {
        let mut sim = Sim::new_eastbrook("Qcol", PlayerClass::Warrior);
        // complete report + wolves via log injection so this test does not fight wolves
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Completed,
                counts: vec![1],
                completed_tick: 0,
            });
            log.quest_log.push(QuestProgress {
                quest_id: "wolves_at_the_gate".into(),
                state: QuestState::Completed,
                counts: vec![3],
                completed_tick: 0,
            });
        }
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "boar_tusks".into(),
            },
        );
        sim.grant_item(sim.player_id, "boar_tusk", 2).unwrap();
        let ready = log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && q.state == "ready");
        assert!(ready);
        sim.interact(
            alden,
            InteractAction::TurnInQuest {
                quest_id: "boar_tusks".into(),
                reward_choice: None,
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && q.state == "completed"));
        assert_eq!(
            crate::inventory::player_item_count(&sim.world, sim.player_id, "boar_tusk"),
            0
        );
    }

    #[test]
    fn accept_rejects_already_active_or_completed() {
        let mut sim = Sim::new_eastbrook("Qdup", PlayerClass::Warrior);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        let mut events = Vec::new();
        assert!(
            !accept_quest(
                &mut sim.world,
                sim.player_id,
                "report_to_alden",
                "town_crier",
                &mut events,
            ),
            "re-accept while active must fail"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You have already accepted this quest."
        )));

        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            if let Some(qp) = log
                .quest_log
                .iter_mut()
                .find(|q| q.quest_id == "report_to_alden")
            {
                qp.state = QuestState::Completed;
            }
        }
        events.clear();
        assert!(
            !accept_quest(
                &mut sim.world,
                sim.player_id,
                "report_to_alden",
                "town_crier",
                &mut events,
            ),
            "re-accept while completed must fail"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "You have already completed this quest."
        )));
    }

    #[test]
    fn turn_in_unknown_quest_id_is_silent() {
        let mut sim = Sim::new_eastbrook("Qunk", PlayerClass::Warrior);
        place_at_template(&mut sim, "captain_alden");
        let mut events = Vec::new();
        assert!(!turn_in_quest(
            &mut sim.world,
            sim.player_id,
            "not_a_real_quest",
            "captain_alden",
            sim.tick,
            None,
            &mut events,
        ));
        assert!(
            events.is_empty(),
            "unknown quest id must not toast: {:?}",
            events
        );
    }

    #[test]
    fn turn_in_rejects_wrong_npc() {
        let mut sim = Sim::new_eastbrook("Qwrong", PlayerClass::Warrior);
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Ready,
                counts: vec![1],
                completed_tick: 0,
            });
        }
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
                reward_choice: None,
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "ready"));
    }

    #[test]
    fn ready_toast_on_last_objective() {
        let mut sim = Sim::new_eastbrook("Qready", PlayerClass::Warrior);
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "report_to_alden".into(),
                state: QuestState::Active,
                counts: vec![0],
                completed_tick: 0,
            });
        }
        let mut events = Vec::new();
        on_talked_to(&mut sim.world, sim.player_id, "captain_alden", &mut events);
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Ready to turn in: Report to Alden"
        )));
    }

    fn complete_in_log(sim: &mut Sim, quest_id: &str, counts: Vec<u32>) {
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: quest_id.into(),
                state: QuestState::Completed,
                counts,
                completed_tick: 0,
            });
        }
    }

    #[test]
    fn abandon_removes_active_and_rejects_completed() {
        let mut sim = Sim::new_eastbrook("Qab", PlayerClass::Warrior);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        sim.interact(
            sim.player_id,
            InteractAction::AbandonQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        assert!(log_entries(&sim).is_empty());

        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        let mut events = Vec::new();
        assert!(!abandon_quest(
            &mut sim.world,
            sim.player_id,
            "report_to_alden",
            &mut events,
        ));
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Toast { message } if message == "Nothing to abandon."
        )));
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "report_to_alden" && q.state == "completed"));
    }

    #[test]
    fn share_quest_in_party_range() {
        let mut sim = Sim::new_eastbrook("Alice", PlayerClass::Warrior);
        let alice = sim.player_id;
        let bob = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
        let _ = sim.party_invite(alice, "Bob");
        let _ = sim.party_accept(bob);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        woc_protocol::WorldHost::interact(
            &mut sim,
            alice,
            alice,
            InteractAction::ShareQuest {
                quest_id: "report_to_alden".into(),
            },
        );
        let bob_log = quest_log_entries(&sim.world, bob);
        assert!(
            bob_log.iter().any(|q| q.quest_id == "report_to_alden"),
            "bob should receive shared quest: {bob_log:?}"
        );
    }

    #[test]
    fn daily_quest_resets_after_epoch() {
        let mut sim = Sim::new_eastbrook("Qday", PlayerClass::Warrior);
        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        complete_in_log(&mut sim, "wolves_at_the_gate", vec![3]);
        if let Some(log) = sim.world.get_mut::<QuestLog>(sim.player_id) {
            log.quest_log.push(QuestProgress {
                quest_id: "wolf_patrol".into(),
                state: QuestState::Completed,
                counts: vec![2],
                completed_tick: 0,
            });
        }
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "wolf_patrol".into(),
            },
        );
        assert!(
            !log_entries(&sim)
                .iter()
                .any(|q| q.quest_id == "wolf_patrol" && q.state == "active"),
            "same-epoch daily must stay completed"
        );

        sim.tick = DAILY_PERIOD_TICKS;
        let _ = sim.tick_all();
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "wolf_patrol".into(),
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "wolf_patrol" && q.state != "completed"));
    }

    #[test]
    fn explore_credits_when_player_enters_radius() {
        let mut sim = Sim::new_eastbrook("Qexp", PlayerClass::Warrior);
        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        place_at_template(&mut sim, "town_crier");
        let crier = find_template(&sim, "town_crier");
        sim.interact(
            crier,
            InteractAction::AcceptQuest {
                quest_id: "scout_north_road".into(),
            },
        );
        let y = crate::ecs::spawn::ground_at(-8.0, 40.0);
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = -8.0;
            t.z = 40.0;
            t.y = y;
        }
        let _ = sim.tick_all();
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "scout_north_road" && q.state == "ready"));
    }

    #[test]
    fn escort_arrives_and_death_fails() {
        let mut sim = Sim::new_eastbrook("Qesc", PlayerClass::Warrior);
        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        complete_in_log(&mut sim, "wolves_at_the_gate", vec![3]);
        complete_in_log(&mut sim, "boar_tusks", vec![2]);
        let y = crate::ecs::spawn::ground_at(-8.0, 50.0);
        if let Some(t) = sim.world.get_mut::<Transform>(sim.player_id) {
            t.x = -8.0;
            t.z = 50.0;
            t.y = y;
        }
        place_at_template(&mut sim, "captain_alden");
        // accept needs to be in range of Alden — teleport player back for accept
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "courier_to_the_gate".into(),
            },
        );
        let courier = find_template(&sim, "eastbrook_courier");
        let y = crate::ecs::spawn::ground_at(-8.0, 50.0);
        if let Some(t) = sim.world.get_mut::<Transform>(courier) {
            t.x = -8.0;
            t.z = 50.0;
            t.y = y;
        }
        let _ = sim.tick_all();
        assert!(
            log_entries(&sim)
                .iter()
                .any(|q| q.quest_id == "courier_to_the_gate" && q.state == "ready"),
            "escort at dest should ready: {:?}",
            log_entries(&sim)
        );

        let mut sim = Sim::new_eastbrook("Qesc2", PlayerClass::Warrior);
        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        complete_in_log(&mut sim, "wolves_at_the_gate", vec![3]);
        complete_in_log(&mut sim, "boar_tusks", vec![2]);
        place_at_template(&mut sim, "captain_alden");
        let alden = find_template(&sim, "captain_alden");
        sim.interact(
            alden,
            InteractAction::AcceptQuest {
                quest_id: "courier_to_the_gate".into(),
            },
        );
        let courier = find_template(&sim, "eastbrook_courier");
        if let Some(h) = sim.world.get_mut::<Health>(courier) {
            h.hp = 0.0;
            h.alive = false;
        }
        let _ = sim.tick_all();
        assert!(
            !log_entries(&sim)
                .iter()
                .any(|q| q.quest_id == "courier_to_the_gate"),
            "dead escort must fail the quest"
        );
    }

    #[test]
    fn choice_reward_requires_index() {
        let mut sim = Sim::new_eastbrook("Qch", PlayerClass::Warrior);
        complete_in_log(&mut sim, "report_to_alden", vec![1]);
        place_at_template(&mut sim, "trader_wilkes");
        let wilkes = find_template(&sim, "trader_wilkes");
        sim.interact(
            wilkes,
            InteractAction::AcceptQuest {
                quest_id: "arms_of_the_watch".into(),
            },
        );
        on_talked_to(
            &mut sim.world,
            sim.player_id,
            "captain_alden",
            &mut Vec::new(),
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "arms_of_the_watch" && q.state == "ready"));
        sim.interact(
            wilkes,
            InteractAction::TurnInQuest {
                quest_id: "arms_of_the_watch".into(),
                reward_choice: None,
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "arms_of_the_watch" && q.state == "ready"));
        sim.interact(
            wilkes,
            InteractAction::TurnInQuest {
                quest_id: "arms_of_the_watch".into(),
                reward_choice: Some(0),
            },
        );
        assert!(log_entries(&sim)
            .iter()
            .any(|q| q.quest_id == "arms_of_the_watch" && q.state == "completed"));
        assert!(
            crate::inventory::player_item_count(&sim.world, sim.player_id, "travelers_ration") >= 1
        );
    }
}
