//! Intent collection, camera look, interact keys.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_content::talents::talents_for_class;
use woc_content::{can_equip, item, npc, ItemKind, PlayerClass};
use woc_protocol::{
    AbilitySlot, EntityId, EntityKind, EquipSlot, InteractAction, PlayerIntent, QuestLogEntry,
    TickSnapshot, WsClientMsg,
};
use woc_sim::quests::npc_quest_offers;
use woc_sim::targeting::tab_target_pose;

use crate::hud::{
    cycle_duration_hours, filtered_market, first_consumable_bag_stack, first_equippable_bag_stack,
    first_junk_bag_stack, first_listable_bag_stack, listing_min_bid, UiFlags, MARKET_PAGE_SIZE,
};
use crate::GameHost;

pub(crate) fn grab_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut host: ResMut<GameHost>,
    ui: Res<UiFlags>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        // Close map / guild first; otherwise clear combat target / stop AA and release cursor.
        if ui.show_map || ui.show_guild {
            // panel close handled in handle_interact_keys
        } else {
            host.pending_intent.clear_target = true;
            host.local_auto_attack = false;
            if let Some(sim) = host.sim.as_mut() {
                sim.clear_target();
            }
            host.snapshot.target_id = None;
            host.snapshot.auto_attack = false;
        }
        host.cursor_grabbed = false;
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
    if mouse.just_pressed(MouseButton::Right) {
        host.cursor_grabbed = true;
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

pub(crate) fn camera_look(mut motion: EventReader<MouseMotion>, mut host: ResMut<GameHost>) {
    if !host.cursor_grabbed {
        motion.clear();
        return;
    }
    for ev in motion.read() {
        host.look_yaw -= ev.delta.x * 0.0025;
        host.look_pitch = (host.look_pitch - ev.delta.y * 0.0025).clamp(-1.2, 0.2);
    }
}

fn ability_slot_from_keys(keys: &ButtonInput<KeyCode>) -> Option<AbilitySlot> {
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        return Some(AbilitySlot::Primary);
    }
    if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        return Some(AbilitySlot::Slot2);
    }
    if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        return Some(AbilitySlot::Slot3);
    }
    if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
        return Some(AbilitySlot::Slot4);
    }
    if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
        return Some(AbilitySlot::Slot5);
    }
    None
}

fn tab_cycle_from_snapshot(snap: &TickSnapshot, facing: f32) -> Option<EntityId> {
    let player = snap.entities.iter().find(|e| e.id == snap.player_id)?;
    if !player.alive {
        return None;
    }
    let candidates: Vec<(EntityId, f32, f32)> = snap
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Mob && e.alive)
        .map(|e| (e.id, e.x, e.z))
        .collect();
    tab_target_pose(player.x, player.z, facing, snap.target_id, &candidates)
}

pub(crate) fn collect_intent(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut host: ResMut<GameHost>,
    ui: Res<UiFlags>,
) {
    // Preserve one-shot clear_target from Esc until the next sim/net step consumes it.
    let clear_target = host.pending_intent.clear_target;
    let mut intent = PlayerIntent {
        facing: host.look_yaw,
        clear_target,
        ..default()
    };
    // The guild compose line and the market search field own the keyboard.
    let typing = ui.show_guild || ui.market_searching;
    let mut mx = 0.0;
    let mut mz = 0.0;
    if !typing {
        if keys.pressed(KeyCode::KeyW) {
            mz += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            mz -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            mx -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            mx += 1.0;
        }
        if keys.just_pressed(KeyCode::Space) || keys.pressed(KeyCode::Space) {
            // Held Space: jump / swim hop / fly ascend (matches upstream MoveInput.jump).
            intent.jump = true;
        }
        if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
            intent.descend = true;
        }
        if keys.just_pressed(KeyCode::KeyV) && !ui.show_bags {
            intent.fly_toggle = true;
        }
    }
    intent.move_x = mx;
    intent.move_z = mz;
    // Digit keys: talents, loot rolls, bank withdraw, or abilities.
    let loot_rolling = host.snapshot.pending_loot.iter().any(|p| !p.rolled);
    let choice_turn_in = nearest_npc_template(&host.snapshot, 5.0)
        .and_then(|(_, tid)| choice_turn_in_for_npc(&tid, &host.snapshot.quest_log))
        .is_some();
    if !ui.show_talents
        && !ui.show_bank
        && !ui.show_guild
        && !loot_rolling
        && !ui.show_bags
        && !ui.show_character
        && !choice_turn_in
    {
        intent.ability = ability_slot_from_keys(&keys);
    }

    if keys.just_pressed(KeyCode::Tab) {
        if let Some(id) = tab_cycle_from_snapshot(&host.snapshot, host.look_yaw) {
            intent.target_id = Some(id);
            host.snapshot.target_id = Some(id);
            if let Some(sim) = host.sim.as_mut() {
                sim.set_player_target(Some(id));
            }
        }
    }

    let bags_consume_f = ui.show_bags;
    let class_id = host.snapshot.progress.class_id.as_str();
    let form_f = matches!(class_id, "warrior" | "shaman" | "druid")
        && keys.just_pressed(KeyCode::KeyF)
        && !bags_consume_f;
    if !typing && mouse.just_pressed(MouseButton::Left) {
        let player_pos = host.player_snap().map(|p| (p.x, p.z));
        let player_id = host.snapshot.player_id;
        let mut best: Option<(EntityId, EntityKind, f32)> = None;
        if let Some((px, pz)) = player_pos {
            for e in &host.snapshot.entities {
                let is_other_player = e.kind == EntityKind::Player && e.id != player_id && e.alive;
                let is_mob = e.kind == EntityKind::Mob && e.alive;
                if !is_other_player && !is_mob {
                    continue;
                }
                let dx = e.x - px;
                let dz = e.z - pz;
                let d = (dx * dx + dz * dz).sqrt();
                if d < 25.0 && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                    best = Some((e.id, e.kind, d));
                }
            }
        }
        if let Some((id, kind, _)) = best {
            intent.target_id = Some(id);
            if kind == EntityKind::Player {
                host.snapshot.target_id = Some(id);
                host.local_auto_attack = false;
                if let Some(sim) = host.sim.as_mut() {
                    sim.set_player_target(Some(id));
                }
            } else {
                intent.attack = true;
                host.local_auto_attack = true;
            }
        } else {
            intent.attack = true;
            host.local_auto_attack = true;
        }
    } else if !typing && keys.pressed(KeyCode::KeyF) && !bags_consume_f && !form_f {
        intent.attack = true;
        host.local_auto_attack = true;
        if let Some(p) = host.player_snap() {
            let (px, pz) = (p.x, p.z);
            let mut best: Option<(EntityId, f32)> = None;
            for e in &host.snapshot.entities {
                if e.kind != EntityKind::Mob || !e.alive {
                    continue;
                }
                let dx = e.x - px;
                let dz = e.z - pz;
                let d = (dx * dx + dz * dz).sqrt();
                if d < 25.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((e.id, d));
                }
            }
            if let Some((id, _)) = best {
                intent.target_id = Some(id);
            }
        }
    }
    // Offline: sticky auto-attack lives on the sim entity.
    if !host.is_online() {
        if let Some(sim) = host.sim.as_ref() {
            if sim.player_auto_attack() {
                intent.attack = true;
                intent.target_id = intent.target_id.or_else(|| sim.player_target());
            }
        }
    } else if host.local_auto_attack {
        intent.attack = true;
        intent.target_id = intent.target_id.or(host.snapshot.target_id);
    }
    host.pending_intent = intent;
}

fn targeted_other_member_name(snap: &TickSnapshot) -> Option<String> {
    let tid = snap.target_id?;
    snap.party_members
        .iter()
        .find(|m| m.id == tid && m.id != snap.player_id)
        .map(|m| m.name.clone())
}

fn first_available_talent_id(snap: &TickSnapshot) -> Option<String> {
    let class = snap.progress.class_id.as_str();
    let ranks: Vec<(String, u32)> = snap
        .talents
        .iter()
        .map(|t| (t.talent_id.clone(), t.rank))
        .collect();
    talents_for_class(class)
        .find(|def| {
            let rank = ranks
                .iter()
                .find(|(id, _)| id == def.id)
                .map(|(_, r)| *r)
                .unwrap_or(0);
            rank < def.max_rank && woc_content::talent_tier_unlocked(class, &ranks, def)
        })
        .map(|def| def.id.to_string())
}

fn talent_id_at_index(snap: &TickSnapshot, index: usize) -> Option<String> {
    talents_for_class(&snap.progress.class_id)
        .nth(index)
        .map(|def| def.id.to_string())
}

fn try_learn_talent(host: &mut GameHost, player_id: EntityId, talent_id: String) {
    if host.snapshot.talent_points == 0 {
        host.recent_toasts
            .push(("No unspent talent points.".into(), 2.0));
        return;
    }
    host.interact(
        player_id,
        InteractAction::LearnTalent {
            talent_id: talent_id.clone(),
        },
    );
    host.recent_toasts
        .push((format!("Learning talent: {talent_id}"), 2.0));
}

fn local_pet_alive(snap: &TickSnapshot) -> bool {
    snap.entities
        .iter()
        .any(|e| e.kind == EntityKind::Pet && e.alive)
}

fn vendor_session_open(snap: &TickSnapshot) -> bool {
    snap.open_vendor.is_some()
        || snap.open_npc.as_ref().is_some_and(|npc| {
            npc.services
                .iter()
                .any(|service| service.as_str() == "vendor")
        })
}

pub(crate) fn tracked_quest_id(log: &[QuestLogEntry]) -> Option<&str> {
    log.iter()
        .find(|e| e.state.eq_ignore_ascii_case("active") || e.state.eq_ignore_ascii_case("ready"))
        .map(|e| e.quest_id.as_str())
}

pub(crate) fn nearest_npc_template(snap: &TickSnapshot, range: f32) -> Option<(EntityId, String)> {
    let player = snap.entities.iter().find(|e| e.id == snap.player_id)?;
    let mut best: Option<(EntityId, f32, String)> = None;
    for e in &snap.entities {
        if e.kind != EntityKind::Npc || !e.alive {
            continue;
        }
        let Some(template_id) = e.template_id.as_ref() else {
            continue;
        };
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d <= range && best.as_ref().map(|(_, bd, _)| d < *bd).unwrap_or(true) {
            best = Some((e.id, d, template_id.clone()));
        }
    }
    best.map(|(id, _, tid)| (id, tid))
}

pub(crate) fn choice_turn_in_for_npc(
    template_id: &str,
    log: &[QuestLogEntry],
) -> Option<&'static woc_content::QuestDef> {
    npc_quest_offers(template_id, log)
        .turn_in
        .into_iter()
        .find(|q| !q.reward.choices.is_empty())
}

pub(crate) fn quest_interact_actions(
    template_id: &str,
    log: &[QuestLogEntry],
) -> Vec<InteractAction> {
    let offers = npc_quest_offers(template_id, log);
    let mut out = Vec::new();
    for q in offers.turn_in {
        if !q.reward.choices.is_empty() {
            continue;
        }
        out.push(InteractAction::TurnInQuest {
            quest_id: q.id.to_string(),
            reward_choice: None,
        });
    }
    for q in offers.accept {
        out.push(InteractAction::AcceptQuest {
            quest_id: q.id.to_string(),
        });
    }
    out
}

fn market_search_keys() -> impl Iterator<Item = (KeyCode, char)> {
    [
        (KeyCode::KeyA, 'a'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'),
        (KeyCode::KeyE, 'e'),
        (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'),
        (KeyCode::KeyH, 'h'),
        (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'),
        (KeyCode::KeyK, 'k'),
        (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'),
        (KeyCode::KeyN, 'n'),
        (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'),
        (KeyCode::KeyQ, 'q'),
        (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'),
        (KeyCode::KeyT, 't'),
        (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'),
        (KeyCode::KeyW, 'w'),
        (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'),
        (KeyCode::KeyZ, 'z'),
        (KeyCode::Space, ' '),
        (KeyCode::Minus, '-'),
    ]
    .into_iter()
}

pub(crate) fn handle_interact_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut host: ResMut<GameHost>,
    mut ui: ResMut<UiFlags>,
) {
    if keys.just_pressed(KeyCode::KeyB) && !ui.show_market && !ui.show_guild {
        ui.show_bags = !ui.show_bags;
    }
    if keys.just_pressed(KeyCode::KeyL) && !ui.show_market && !ui.show_guild {
        ui.show_quests = !ui.show_quests;
    }
    if keys.just_pressed(KeyCode::KeyC) && !ui.show_guild {
        ui.show_character = !ui.show_character;
        if ui.show_character {
            ui.show_bags = false;
            ui.show_talents = false;
            ui.show_bank = false;
            ui.show_mail = false;
            ui.show_market = false;
            ui.show_map = false;
            ui.show_guild = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyN) && !ui.show_guild {
        ui.show_talents = !ui.show_talents;
        if ui.show_talents {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyK) && !ui.show_guild {
        ui.show_bank = !ui.show_bank;
        if ui.show_bank {
            ui.show_character = false;
            ui.show_map = false;
            ui.show_guild = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyI) && !ui.show_guild {
        ui.show_mail = !ui.show_mail;
        if ui.show_mail {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyM) && !ui.show_guild {
        ui.show_map = !ui.show_map;
        if ui.show_map {
            ui.show_character = false;
            ui.show_talents = false;
            ui.show_bank = false;
            ui.show_mail = false;
            ui.show_market = false;
            ui.show_guild = false;
        }
    }
    if !ui.show_bank && !ui.show_guild && keys.just_pressed(KeyCode::KeyJ) {
        ui.show_guild = true;
        ui.show_character = false;
        ui.show_map = false;
        ui.show_bank = false;
        ui.show_mail = false;
        ui.show_market = false;
        return;
    }
    if keys.just_pressed(KeyCode::KeyU) && !ui.show_guild {
        ui.show_market = !ui.show_market;
        if ui.show_market {
            ui.show_character = false;
            ui.show_map = false;
            ui.market_page = 0;
        } else {
            ui.market_searching = false;
        }
    }
    if ui.show_market && keys.just_pressed(KeyCode::Escape) {
        if ui.market_searching {
            ui.market_searching = false;
        } else if !ui.market_filter.is_empty() {
            ui.market_filter.clear();
            ui.market_page = 0;
        }
    }
    if keys.just_pressed(KeyCode::Escape) && ui.show_guild {
        ui.show_guild = false;
        ui.guild_compose.clear();
    }
    if keys.just_pressed(KeyCode::Escape) && ui.show_map {
        ui.show_map = false;
    }

    // Compose owns the keyboard while the guild panel is open: no gameplay,
    // no other panel's hotkeys.
    if ui.show_guild {
        handle_guild_panel_keys(&keys, &mut host, &mut ui);
        return;
    }

    let pending = !host.snapshot.pending_invite_from.is_empty();
    let ready_prompt = host
        .snapshot
        .ready_check
        .as_ref()
        .is_some_and(|r| !r.you_responded);

    if pending && !ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        host.send_party(WsClientMsg::PartyAccept);
    } else if ready_prompt && !ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        host.send_party(WsClientMsg::PartyReadyRespond { ready: true });
    }

    if pending && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        host.send_party(WsClientMsg::PartyDecline);
    } else if ready_prompt && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        host.send_party(WsClientMsg::PartyReadyRespond { ready: false });
    } else if !pending && !ready_prompt && !ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        ui.show_party = !ui.show_party;
        if ui.show_party {
            ui.show_character = false;
            ui.show_map = false;
        }
    }

    if !ui.show_bank && keys.just_pressed(KeyCode::KeyG) {
        if let Some(tid) = host.snapshot.target_id {
            let invite_name = host.snapshot.entities.iter().find_map(|e| {
                if e.id == tid && e.kind == EntityKind::Player && e.id != host.snapshot.player_id {
                    Some(e.name.clone())
                } else {
                    None
                }
            });
            if let Some(name) = invite_name {
                host.send_party(WsClientMsg::PartyInvite { name });
            }
        }
    }

    if ui.show_party && keys.just_pressed(KeyCode::KeyX) {
        host.send_party(WsClientMsg::PartyLeave);
    }
    if ui.show_party && keys.just_pressed(KeyCode::KeyY) {
        if let Some(name) = targeted_other_member_name(&host.snapshot) {
            host.send_party(WsClientMsg::PartyPromote { name });
        }
    }
    if ui.show_party && keys.just_pressed(KeyCode::Minus) {
        if let Some(name) = targeted_other_member_name(&host.snapshot) {
            host.send_party(WsClientMsg::PartyKick { name });
        }
    }
    if ui.show_party && keys.just_pressed(KeyCode::KeyR) {
        host.send_party(WsClientMsg::PartyReadyCheck);
    }
    if ui.show_party && keys.just_pressed(KeyCode::Backspace) {
        host.send_party(WsClientMsg::PartyDisband);
    }
    if ui.show_party && keys.just_pressed(KeyCode::Equal) {
        if host.snapshot.party_kind == "raid" {
            host.send_party(WsClientMsg::ConvertToParty);
        } else {
            host.send_party(WsClientMsg::ConvertToRaid);
        }
    }

    let player_id = host.snapshot.player_id;
    if ui.show_quests && !ui.show_talents {
        if keys.just_pressed(KeyCode::KeyX) {
            if let Some(qid) = tracked_quest_id(&host.snapshot.quest_log).map(str::to_string) {
                host.interact(player_id, InteractAction::AbandonQuest { quest_id: qid });
            }
        }
        if keys.just_pressed(KeyCode::KeyY) {
            if let Some(qid) = tracked_quest_id(&host.snapshot.quest_log).map(str::to_string) {
                host.interact(player_id, InteractAction::ShareQuest { quest_id: qid });
            }
        }
    }
    if ui.show_talents {
        let slot = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
            Some(0usize)
        } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
            Some(1)
        } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
            Some(2)
        } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
            Some(3)
        } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
            Some(4)
        } else {
            None
        };
        if let Some(idx) = slot {
            if let Some(talent_id) = talent_id_at_index(&host.snapshot, idx) {
                try_learn_talent(&mut host, player_id, talent_id);
            } else {
                host.recent_toasts
                    .push((format!("No talent in slot {}.", idx + 1), 2.0));
            }
        }
    }
    if ui.show_talents && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyY)) {
        if let Some(talent_id) = first_available_talent_id(&host.snapshot) {
            try_learn_talent(&mut host, player_id, talent_id);
        } else if host.snapshot.talent_points == 0 {
            host.recent_toasts
                .push(("No unspent talent points.".into(), 2.0));
        } else {
            host.recent_toasts
                .push(("No available class talent.".into(), 2.0));
        }
    }
    if ui.show_talents && keys.just_pressed(KeyCode::KeyR) {
        host.interact(player_id, InteractAction::RespecTalents);
        host.recent_toasts.push(("Respeccing talents.".into(), 2.0));
    }
    if !ui.show_talents && !ui.show_party && keys.just_pressed(KeyCode::KeyR) {
        if let Some(npc) = host
            .snapshot
            .open_npc
            .as_ref()
            .filter(|npc| npc.can_repair)
            .cloned()
        {
            host.interact(npc.npc_id, InteractAction::RepairAll);
            host.recent_toasts.push(("Repairing gear.".into(), 2.0));
        }
    }

    // Pet toggle (hunter/warlock): T — when mail/guild panels are closed so P stays mail-collect.
    if !ui.show_mail && !ui.show_guild && keys.just_pressed(KeyCode::KeyT) {
        if local_pet_alive(&host.snapshot) {
            host.interact(player_id, InteractAction::DismissPet);
            host.recent_toasts.push(("Dismissing pet…".into(), 1.5));
        } else {
            host.interact(player_id, InteractAction::SummonPet);
            host.recent_toasts.push(("Summoning pet…".into(), 1.5));
        }
    }
    // Rogue stealth (Z). Other classes toast from the sim.
    if !ui.show_mail
        && !ui.show_bank
        && !ui.show_market
        && !ui.show_talents
        && keys.just_pressed(KeyCode::KeyZ)
    {
        host.interact(player_id, InteractAction::ToggleStealth);
    }
    // Warrior stance / shaman+druid form (F). Held F still attacks for other classes.
    if !ui.show_mail
        && !ui.show_bank
        && !ui.show_market
        && !ui.show_talents
        && !ui.show_bags
        && keys.just_pressed(KeyCode::KeyF)
    {
        match host.snapshot.progress.class_id.as_str() {
            "warrior" => host.interact(player_id, InteractAction::CycleStance),
            "shaman" | "druid" => host.interact(player_id, InteractAction::ToggleForm),
            _ => {}
        }
    }
    if ui.show_bank && keys.just_pressed(KeyCode::KeyG) {
        if let Some((bag_slot, count, item_id)) = first_junk_bag_stack(&host.snapshot) {
            host.interact(player_id, InteractAction::BankDeposit { bag_slot, count });
            host.recent_toasts
                .push((format!("Depositing {count}×{item_id}."), 2.0));
        } else {
            host.recent_toasts
                .push(("No junk stack in bags.".into(), 2.0));
        }
    }
    if ui.show_bank && keys.just_pressed(KeyCode::KeyH) {
        if let Some(stack) = host.snapshot.bank.first().cloned() {
            host.interact(
                player_id,
                InteractAction::BankWithdraw {
                    bank_slot: stack.slot,
                    count: stack.count,
                },
            );
            host.recent_toasts.push((
                format!("Withdrawing {}×{}.", stack.count, stack.item_id),
                2.0,
            ));
        } else {
            host.recent_toasts.push(("Bank is empty.".into(), 2.0));
        }
    }
    if !ui.show_bank && keys.just_pressed(KeyCode::KeyH) {
        host.interact(player_id, InteractAction::UseHearthstone);
        host.recent_toasts.push(("Using hearthstone.".into(), 2.0));
    }
    if ui.show_bank {
        let bank_idx = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1)
        {
            Some(0usize)
        } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
            Some(1)
        } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
            Some(2)
        } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
            Some(3)
        } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
            Some(4)
        } else if keys.just_pressed(KeyCode::Digit6) || keys.just_pressed(KeyCode::Numpad6) {
            Some(5)
        } else if keys.just_pressed(KeyCode::Digit7) || keys.just_pressed(KeyCode::Numpad7) {
            Some(6)
        } else if keys.just_pressed(KeyCode::Digit8) || keys.just_pressed(KeyCode::Numpad8) {
            Some(7)
        } else if keys.just_pressed(KeyCode::Digit9) || keys.just_pressed(KeyCode::Numpad9) {
            Some(8)
        } else {
            None
        };
        if let Some(idx) = bank_idx {
            if let Some(stack) = host.snapshot.bank.get(idx).cloned() {
                host.interact(
                    player_id,
                    InteractAction::BankWithdraw {
                        bank_slot: stack.slot,
                        count: stack.count,
                    },
                );
                host.recent_toasts.push((
                    format!("Withdrawing {}×{}.", stack.count, stack.item_id),
                    2.0,
                ));
            }
        }
    }
    if ui.show_bank && keys.just_pressed(KeyCode::KeyJ) {
        let amount = host.snapshot.progress.copper;
        host.interact(player_id, InteractAction::BankDepositCopper { amount });
        host.recent_toasts
            .push((format!("Depositing {amount}c to vault."), 2.0));
    }
    if ui.show_bank && keys.just_pressed(KeyCode::KeyY) {
        let amount = host.snapshot.bank_copper;
        host.interact(player_id, InteractAction::BankWithdrawCopper { amount });
        host.recent_toasts
            .push((format!("Withdrawing {amount}c from vault."), 2.0));
    }
    if ui.show_mail && !ui.show_guild && keys.just_pressed(KeyCode::KeyP) {
        if let Some(mail) = host.snapshot.mail.first() {
            let mail_id = mail.id;
            host.interact(player_id, InteractAction::MailCollect { mail_id });
            host.recent_toasts
                .push((format!("Collecting mail #{mail_id}."), 2.0));
        } else {
            host.recent_toasts.push(("Inbox is empty.".into(), 2.0));
        }
    }
    if ui.show_market && ui.market_searching {
        if keys.just_pressed(KeyCode::Backspace) {
            ui.market_filter.pop();
            ui.market_page = 0;
        } else {
            for (code, ch) in market_search_keys() {
                if keys.just_pressed(code) {
                    ui.market_filter.push(ch);
                    ui.market_page = 0;
                }
            }
        }
    }
    if ui.show_market && !ui.market_searching && keys.just_pressed(KeyCode::Slash) {
        ui.market_searching = true;
    }
    if ui.show_market && keys.just_pressed(KeyCode::BracketLeft) {
        ui.market_page = ui.market_page.saturating_sub(1);
    }
    if ui.show_market && keys.just_pressed(KeyCode::BracketRight) {
        let pages = filtered_market(&host.snapshot, &ui.market_filter)
            .len()
            .div_ceil(MARKET_PAGE_SIZE)
            .max(1);
        ui.market_page = (ui.market_page + 1).min(pages.saturating_sub(1));
    }
    if ui.show_market && keys.just_pressed(KeyCode::Comma) {
        ui.market_duration_hours = cycle_duration_hours(ui.market_duration_hours, false);
    }
    if ui.show_market && keys.just_pressed(KeyCode::Period) {
        ui.market_duration_hours = cycle_duration_hours(ui.market_duration_hours, true);
    }
    if ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        if let Some(listing) = host
            .snapshot
            .market
            .iter()
            .find(|l| !l.mine && l.price <= host.snapshot.progress.copper)
            .cloned()
        {
            host.interact(
                player_id,
                InteractAction::MarketBuy {
                    listing_id: listing.id,
                },
            );
            host.recent_toasts.push((
                format!("Buying listing #{} for {}c.", listing.id, listing.price),
                2.0,
            ));
        } else {
            host.recent_toasts
                .push(("No affordable market listings.".into(), 2.0));
        }
    }
    if ui.show_market && keys.just_pressed(KeyCode::KeyL) {
        if let Some((bag_slot, count, item_id, price)) = first_listable_bag_stack(&host.snapshot) {
            host.interact(
                player_id,
                InteractAction::MarketList {
                    bag_slot,
                    count,
                    price,
                    start_bid: (price / 2).max(1),
                    duration_hours: ui.market_duration_hours,
                },
            );
            host.recent_toasts
                .push((format!("Listing 1×{item_id} for {price}c."), 2.0));
        } else {
            host.recent_toasts
                .push(("Nothing listable in bags.".into(), 2.0));
        }
    }
    if ui.show_market && keys.just_pressed(KeyCode::KeyX) {
        if let Some(listing) = host.snapshot.market.iter().find(|l| l.mine).cloned() {
            host.interact(
                player_id,
                InteractAction::MarketCancel {
                    listing_id: listing.id,
                },
            );
            host.recent_toasts
                .push((format!("Cancelling listing #{}.", listing.id), 2.0));
        } else {
            host.recent_toasts
                .push(("You have no listings.".into(), 2.0));
        }
    }
    if ui.show_market && keys.just_pressed(KeyCode::KeyB) {
        if let Some(listing) = filtered_market(&host.snapshot, &ui.market_filter)
            .into_iter()
            .find(|l| {
                !l.mine && listing_min_bid(l).is_some_and(|m| m <= host.snapshot.progress.copper)
            })
            .cloned()
        {
            let amount = listing_min_bid(&listing).unwrap_or(1);
            host.interact(
                player_id,
                InteractAction::MarketBid {
                    listing_id: listing.id,
                    amount,
                },
            );
            host.recent_toasts.push((
                format!("Bidding {amount}c on listing #{}.", listing.id),
                2.0,
            ));
        } else {
            host.recent_toasts
                .push(("No biddable market listings.".into(), 2.0));
        }
    }

    if ui.show_bags && !ui.show_character {
        let bag_slot = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1)
        {
            Some(0u8)
        } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
            Some(1)
        } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
            Some(2)
        } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
            Some(3)
        } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
            Some(4)
        } else if keys.just_pressed(KeyCode::Digit6) || keys.just_pressed(KeyCode::Numpad6) {
            Some(5)
        } else if keys.just_pressed(KeyCode::Digit7) || keys.just_pressed(KeyCode::Numpad7) {
            Some(6)
        } else if keys.just_pressed(KeyCode::Digit8) || keys.just_pressed(KeyCode::Numpad8) {
            Some(7)
        } else if keys.just_pressed(KeyCode::Digit9) || keys.just_pressed(KeyCode::Numpad9) {
            Some(8)
        } else {
            None
        };
        if let Some(slot) = bag_slot {
            if let Some(stack) = host
                .snapshot
                .inventory
                .iter()
                .find(|s| s.slot == slot)
                .cloned()
            {
                if let Some(def) = item(&stack.item_id) {
                    let level = host.snapshot.progress.level;
                    let can_equip_ok = PlayerClass::parse(&host.snapshot.progress.class_id)
                        .map(|class| can_equip(def, class, level).is_ok())
                        .unwrap_or(false);
                    if can_equip_ok {
                        host.interact(
                            player_id,
                            InteractAction::Equip {
                                bag_slot: stack.slot,
                            },
                        );
                        host.recent_toasts
                            .push((format!("Equipping {}.", stack.item_id), 2.0));
                    } else if def.kind == ItemKind::Consumable {
                        host.interact(
                            player_id,
                            InteractAction::UseItem {
                                bag_slot: stack.slot,
                            },
                        );
                        host.recent_toasts
                            .push((format!("Using {}.", stack.item_id), 2.0));
                    } else {
                        host.recent_toasts.push(("Cannot use that.".into(), 2.0));
                    }
                } else {
                    host.recent_toasts.push(("Cannot use that.".into(), 2.0));
                }
            }
        }
    }
    if ui.show_character {
        let equip_idx = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1)
        {
            Some(0usize)
        } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
            Some(1)
        } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
            Some(2)
        } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
            Some(3)
        } else if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5) {
            Some(4)
        } else if keys.just_pressed(KeyCode::Digit6) || keys.just_pressed(KeyCode::Numpad6) {
            Some(5)
        } else if keys.just_pressed(KeyCode::Digit7) || keys.just_pressed(KeyCode::Numpad7) {
            Some(6)
        } else if keys.just_pressed(KeyCode::Digit8) || keys.just_pressed(KeyCode::Numpad8) {
            Some(7)
        } else if keys.just_pressed(KeyCode::Digit9) || keys.just_pressed(KeyCode::Numpad9) {
            Some(8)
        } else if keys.just_pressed(KeyCode::Digit0) || keys.just_pressed(KeyCode::Numpad0) {
            Some(9)
        } else if keys.just_pressed(KeyCode::Minus) {
            Some(10)
        } else if keys.just_pressed(KeyCode::Equal) {
            Some(11)
        } else if keys.just_pressed(KeyCode::BracketLeft) {
            Some(12)
        } else if keys.just_pressed(KeyCode::BracketRight) {
            Some(13)
        } else if keys.just_pressed(KeyCode::Semicolon) {
            Some(14)
        } else if keys.just_pressed(KeyCode::Quote) {
            Some(15)
        } else {
            None
        };
        if let Some(idx) = equip_idx {
            const SLOTS: [EquipSlot; 16] = [
                EquipSlot::MainHand,
                EquipSlot::OffHand,
                EquipSlot::Head,
                EquipSlot::Chest,
                EquipSlot::Legs,
                EquipSlot::Feet,
                EquipSlot::Neck,
                EquipSlot::Finger,
                EquipSlot::Finger2,
                EquipSlot::Shoulder,
                EquipSlot::Back,
                EquipSlot::Wrist,
                EquipSlot::Hands,
                EquipSlot::Waist,
                EquipSlot::Trinket,
                EquipSlot::Trinket2,
            ];
            let equip_slot = SLOTS[idx];
            host.interact(player_id, InteractAction::Unequip { equip_slot });
            host.recent_toasts
                .push((format!("Unequipping {equip_slot:?}."), 2.0));
        }
    }

    // Bags: equip / use / sell junk while vendor open.
    if ui.show_bags && keys.just_pressed(KeyCode::KeyQ) {
        if let Some((bag_slot, item_id)) = first_equippable_bag_stack(&host.snapshot) {
            host.interact(player_id, InteractAction::Equip { bag_slot });
            host.recent_toasts
                .push((format!("Equipping {item_id}."), 2.0));
        } else {
            host.recent_toasts
                .push(("No equippable item in bags.".into(), 2.0));
        }
    }
    if ui.show_bags && keys.just_pressed(KeyCode::KeyF) {
        if let Some((bag_slot, item_id)) = first_consumable_bag_stack(&host.snapshot) {
            host.interact(player_id, InteractAction::UseItem { bag_slot });
            host.recent_toasts.push((format!("Using {item_id}."), 2.0));
        } else {
            host.recent_toasts
                .push(("No consumable in bags.".into(), 2.0));
        }
    }
    if ui.show_bags && vendor_session_open(&host.snapshot) && keys.just_pressed(KeyCode::KeyV) {
        if let Some((bag_slot, count, item_id)) = first_junk_bag_stack(&host.snapshot) {
            host.interact(player_id, InteractAction::Sell { bag_slot, count });
            host.recent_toasts
                .push((format!("Selling {count}×{item_id}."), 2.0));
        } else {
            host.recent_toasts.push(("No junk to sell.".into(), 2.0));
        }
    }

    // Need/Greed: 1/2/3 when a roll is pending (and bank/talents closed).
    let pending = host
        .snapshot
        .pending_loot
        .iter()
        .find(|p| !p.rolled)
        .cloned();
    if let Some(pending) = pending {
        if !ui.show_talents && !ui.show_bank && !ui.show_bags && !ui.show_character {
            if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
                host.interact(
                    player_id,
                    InteractAction::LootNeed {
                        loot_id: pending.loot_id,
                    },
                );
                host.recent_toasts.push(("Rolling Need…".into(), 1.5));
            } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
                host.interact(
                    player_id,
                    InteractAction::LootGreed {
                        loot_id: pending.loot_id,
                    },
                );
                host.recent_toasts.push(("Rolling Greed…".into(), 1.5));
            } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
                host.interact(
                    player_id,
                    InteractAction::LootPass {
                        loot_id: pending.loot_id,
                    },
                );
                host.recent_toasts.push(("Passing on loot…".into(), 1.5));
            }
        }
    }

    let loot_busy = host.snapshot.pending_loot.iter().any(|p| !p.rolled);
    if !loot_busy && !ui.show_talents && !ui.show_bank && !ui.show_bags && !ui.show_character {
        if let Some((nid, template_id)) = nearest_npc_template(&host.snapshot, 5.0) {
            if let Some(def) = choice_turn_in_for_npc(&template_id, &host.snapshot.quest_log) {
                let idx = if keys.just_pressed(KeyCode::Digit1)
                    || keys.just_pressed(KeyCode::Numpad1)
                {
                    Some(0u32)
                } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2)
                {
                    Some(1)
                } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3)
                {
                    Some(2)
                } else {
                    None
                };
                if let Some(reward_choice) = idx {
                    if def.reward.choices.get(reward_choice as usize).is_some() {
                        host.interact(
                            nid,
                            InteractAction::TurnInQuest {
                                quest_id: def.id.to_string(),
                                reward_choice: Some(reward_choice),
                            },
                        );
                    }
                }
            }
        }
    }

    // Party leader loot mode: [ = FFA, ] = Need/Greed
    if host.snapshot.party_id.is_some() {
        if keys.just_pressed(KeyCode::BracketLeft) {
            host.interact(
                player_id,
                InteractAction::SetLootMode { mode: "ffa".into() },
            );
        }
        if keys.just_pressed(KeyCode::BracketRight) {
            host.interact(
                player_id,
                InteractAction::SetLootMode {
                    mode: "need_greed".into(),
                },
            );
        }
    }

    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Some(player) = host.player_snap().cloned() else {
        host.recent_toasts
            .push(("No player yet (waiting for snapshot).".into(), 2.0));
        return;
    };

    // Prefer looting nearby piles / corpses, then NPCs.
    let mut best_loot: Option<(EntityId, f32)> = None;
    for e in &host.snapshot.entities {
        let is_loot = e.kind == EntityKind::Loot && e.alive;
        let is_corpse = e.kind == EntityKind::Mob && !e.alive;
        if !is_loot && !is_corpse {
            continue;
        }
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d < 5.0 && best_loot.map(|(_, bd)| d < bd).unwrap_or(true) {
            best_loot = Some((e.id, d));
        }
    }
    if let Some((lid, _)) = best_loot {
        host.interact(lid, InteractAction::LootCorpse { target_id: lid });
        host.recent_toasts.push(("Looting…".into(), 1.5));
        return;
    }

    let mut best: Option<(EntityId, f32, Option<String>)> = None;
    for e in &host.snapshot.entities {
        if e.kind != EntityKind::Npc || !e.alive {
            continue;
        }
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d < 5.0 && best.as_ref().map(|(_, bd, _)| d < *bd).unwrap_or(true) {
            best = Some((e.id, d, e.template_id.clone()));
        }
    }
    let Some((nid, _, template_id)) = best else {
        host.recent_toasts.push(("No NPC nearby.".into(), 2.0));
        return;
    };

    host.interact(nid, InteractAction::Talk);

    if template_id
        .as_deref()
        .and_then(npc)
        .is_some_and(|d| d.is_auctioneer())
    {
        ui.show_market = true;
        ui.show_character = false;
        ui.show_map = false;
    }
    if template_id
        .as_deref()
        .and_then(npc)
        .is_some_and(|d| d.is_banker())
    {
        ui.show_bank = true;
        ui.show_character = false;
        ui.show_map = false;
    }
    if template_id
        .as_deref()
        .and_then(npc)
        .is_some_and(|d| d.is_mailbox())
    {
        ui.show_mail = true;
        ui.show_character = false;
        ui.show_map = false;
    }

    if let Some(template_id) = template_id.as_deref() {
        for action in quest_interact_actions(template_id, &host.snapshot.quest_log) {
            host.interact(nid, action);
        }
    }
}

fn guild_enter_msg(compose: &str, snap: &TickSnapshot) -> WsClientMsg {
    if snap.guild_invite.is_some() {
        return WsClientMsg::GuildAccept;
    }
    if snap.guild.is_none() {
        return WsClientMsg::GuildCreate {
            name: compose.trim().to_string(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/motd ") {
        return WsClientMsg::GuildSetMotd {
            text: rest.to_string(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/o ") {
        return WsClientMsg::Chat {
            channel: "officer".into(),
            text: rest.to_string(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/invite ") {
        return WsClientMsg::GuildInvite {
            name: rest.trim().to_string(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/kick ") {
        return WsClientMsg::GuildKick {
            name: rest.trim().to_string(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/officer ") {
        return WsClientMsg::GuildSetRank {
            name: rest.trim().to_string(),
            rank: "officer".into(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/member ") {
        return WsClientMsg::GuildSetRank {
            name: rest.trim().to_string(),
            rank: "member".into(),
        };
    }
    if let Some(rest) = compose.strip_prefix("/transfer ") {
        return WsClientMsg::GuildTransferLeader {
            name: rest.trim().to_string(),
        };
    }
    WsClientMsg::Chat {
        channel: "guild".into(),
        text: compose.to_string(),
    }
}

fn targeted_player_name(snap: &TickSnapshot) -> Option<String> {
    let tid = snap.target_id?;
    let entity = snap.entities.iter().find(|e| e.id == tid)?;
    if entity.kind == EntityKind::Player && entity.id != snap.player_id {
        Some(entity.name.clone())
    } else {
        None
    }
}

/// Guild panel keys: Ctrl+letter runs a verb, everything else composes text.
/// `J` opens the panel when closed and types while it is open; Esc closes.
fn handle_guild_panel_keys(keys: &ButtonInput<KeyCode>, host: &mut GameHost, ui: &mut UiFlags) {
    if keys.just_pressed(KeyCode::Enter) {
        let compose = ui.guild_compose.clone();
        let msg = guild_enter_msg(&compose, &host.snapshot);
        host.guild_msg(msg);
        ui.guild_compose.clear();
        return;
    }

    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        if keys.just_pressed(KeyCode::KeyX) && host.snapshot.guild_invite.is_some() {
            host.guild_msg(WsClientMsg::GuildDecline);
        }
        if keys.just_pressed(KeyCode::KeyQ) && host.snapshot.guild.is_some() {
            host.guild_msg(WsClientMsg::GuildLeave);
        }
        if keys.just_pressed(KeyCode::KeyV) && host.snapshot.guild.is_some() {
            if let Some(name) = targeted_player_name(&host.snapshot) {
                host.guild_msg(WsClientMsg::GuildInvite { name });
            }
        }
        if let Some(g) = host.snapshot.guild.as_ref() {
            let rank = g.rank.clone();
            if let Some(name) = targeted_player_name(&host.snapshot) {
                if keys.just_pressed(KeyCode::KeyK) && (rank == "leader" || rank == "officer") {
                    host.guild_msg(WsClientMsg::GuildKick { name: name.clone() });
                }
                if rank == "leader" {
                    if keys.just_pressed(KeyCode::KeyP) {
                        host.guild_msg(WsClientMsg::GuildSetRank {
                            name: name.clone(),
                            rank: "officer".into(),
                        });
                    }
                    if keys.just_pressed(KeyCode::KeyO) {
                        host.guild_msg(WsClientMsg::GuildSetRank {
                            name: name.clone(),
                            rank: "member".into(),
                        });
                    }
                    if keys.just_pressed(KeyCode::KeyT) {
                        host.guild_msg(WsClientMsg::GuildTransferLeader { name });
                    }
                }
            }
            if rank == "leader" && keys.just_pressed(KeyCode::KeyD) {
                host.guild_msg(WsClientMsg::GuildDisband);
            }
        }
        return;
    }

    if keys.just_pressed(KeyCode::Backspace) {
        ui.guild_compose.pop();
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for key in GUILD_COMPOSE_KEYS {
        if keys.just_pressed(key) {
            if let Some(ch) = guild_compose_char_from_key(key, shift) {
                ui.guild_compose.push(ch);
            }
        }
    }
}

const GUILD_COMPOSE_KEYS: [KeyCode; 40] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Space,
    KeyCode::Slash,
    KeyCode::Period,
    KeyCode::Minus,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn guild_compose_char_from_key(key: KeyCode, shift: bool) -> Option<char> {
    let ch = match key {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Space => ' ',
        KeyCode::Slash => '/',
        KeyCode::Period => '.',
        KeyCode::Minus => '-',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        _ => return None,
    };
    Some(if shift { ch.to_ascii_uppercase() } else { ch })
}

#[cfg(test)]
mod tests {
    use super::quest_interact_actions;
    use super::*;
    use woc_protocol::{InvSlotSnapshot, QuestLogEntry, TalentRankSnapshot, TickSnapshot};

    #[test]
    fn e_on_crier_accepts_report_only() {
        let actions = quest_interact_actions("town_crier", &[]);
        assert_eq!(
            actions,
            vec![InteractAction::AcceptQuest {
                quest_id: "report_to_alden".into(),
            }]
        );
    }

    #[test]
    fn e_on_alden_turns_in_then_accepts_next() {
        let log = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "ready".into(),
            counts: vec![1],
        }];
        let actions = quest_interact_actions("captain_alden", &log);
        assert_eq!(
            actions,
            vec![InteractAction::TurnInQuest {
                quest_id: "report_to_alden".into(),
                reward_choice: None,
            }]
        );

        let log = vec![QuestLogEntry {
            quest_id: "report_to_alden".into(),
            state: "completed".into(),
            counts: vec![1],
        }];
        let actions = quest_interact_actions("captain_alden", &log);
        assert_eq!(
            actions,
            vec![InteractAction::AcceptQuest {
                quest_id: "wolves_at_the_gate".into(),
            }]
        );
    }

    #[test]
    fn e_skips_choice_reward_turn_in() {
        let log = vec![QuestLogEntry {
            quest_id: "arms_of_the_watch".into(),
            state: "ready".into(),
            counts: vec![1],
        }];
        let actions = quest_interact_actions("trader_wilkes", &log);
        assert!(
            actions
                .iter()
                .all(|a| !matches!(a, InteractAction::TurnInQuest { .. })),
            "choice turn-in must not auto-fire on E: {actions:?}"
        );
    }

    #[test]
    fn first_available_talent_uses_class_and_skips_max_rank() {
        let mut snap = TickSnapshot::default();
        snap.progress.class_id = "warrior".into();
        for def in talents_for_class("warrior") {
            snap.talents.push(TalentRankSnapshot {
                talent_id: def.id.into(),
                rank: 5,
            });
        }

        assert_eq!(first_available_talent_id(&snap), None);

        snap.talents[0].rank = 4;
        assert_eq!(
            first_available_talent_id(&snap).as_deref(),
            Some("warrior_cruelty")
        );
    }

    #[test]
    fn first_available_skips_locked_tier_two() {
        let mut snap = TickSnapshot::default();
        snap.progress.class_id = "warrior".into();
        snap.talent_points = 1;
        // No points spent — tier 2 vitality must not be "first available".
        assert_eq!(
            first_available_talent_id(&snap).as_deref(),
            Some("warrior_cruelty")
        );
        assert_eq!(
            talent_id_at_index(&snap, 2).as_deref(),
            Some("warrior_vitality")
        );
    }

    #[test]
    fn first_junk_stack_ignores_non_junk_inventory() {
        let mut snap = TickSnapshot::default();
        snap.inventory.push(InvSlotSnapshot {
            slot: 0,
            item_id: "baked_bread".into(),
            count: 2,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
        });
        snap.inventory.push(InvSlotSnapshot {
            slot: 1,
            item_id: "wolf_fang".into(),
            count: 3,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
        });

        assert_eq!(
            first_junk_bag_stack(&snap),
            Some((1, 3, "wolf_fang".into()))
        );
    }

    #[test]
    fn tab_cycle_from_snapshot_picks_nearest_facing() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.entities.push(woc_protocol::EntitySnapshot {
            id: 1,
            kind: EntityKind::Player,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            hp: 100.0,
            hp_max: 100.0,
            level: 1,
            name: "You".into(),
            resource: 0.0,
            resource_max: 100.0,
            alive: true,
            template_id: None,
            on_ground: true,
            flying: false,
            swimming: false,
        });
        snap.entities.push(woc_protocol::EntitySnapshot {
            id: 2,
            kind: EntityKind::Mob,
            x: 5.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            hp: 50.0,
            hp_max: 50.0,
            level: 1,
            name: "Wolf A".into(),
            resource: 0.0,
            resource_max: 0.0,
            alive: true,
            template_id: Some("young_wolf".into()),
            on_ground: true,
            flying: false,
            swimming: false,
        });
        snap.entities.push(woc_protocol::EntitySnapshot {
            id: 3,
            kind: EntityKind::Mob,
            x: 0.0,
            y: 0.0,
            z: 5.0,
            yaw: 0.0,
            hp: 50.0,
            hp_max: 50.0,
            level: 1,
            name: "Wolf B".into(),
            resource: 0.0,
            resource_max: 0.0,
            alive: true,
            template_id: Some("young_wolf".into()),
            on_ground: true,
            flying: false,
            swimming: false,
        });
        let first = tab_cycle_from_snapshot(&snap, 0.0).expect("first");
        snap.target_id = Some(first);
        let second = tab_cycle_from_snapshot(&snap, 0.0).expect("second");
        assert_ne!(first, second);
    }

    fn compose(keys: &[(KeyCode, bool)]) -> String {
        keys.iter()
            .filter_map(|&(key, shift)| guild_compose_char_from_key(key, shift))
            .collect()
    }

    #[test]
    fn compose_alphabet_types_guild_name_and_slash_commands() {
        assert_eq!(
            compose(&[
                (KeyCode::KeyV, true),
                (KeyCode::KeyA, false),
                (KeyCode::KeyL, false),
                (KeyCode::KeyE, false),
                (KeyCode::Space, false),
                (KeyCode::KeyW, true),
                (KeyCode::KeyA, false),
                (KeyCode::KeyT, false),
                (KeyCode::KeyC, false),
                (KeyCode::KeyH, false),
            ]),
            "Vale Watch"
        );
        assert_eq!(
            compose(&[
                (KeyCode::KeyJ, true),
                (KeyCode::KeyA, false),
                (KeyCode::KeyD, false),
                (KeyCode::KeyE, false),
            ]),
            "Jade"
        );
        assert_eq!(
            compose(&[
                (KeyCode::Slash, false),
                (KeyCode::KeyM, false),
                (KeyCode::KeyO, false),
                (KeyCode::KeyT, false),
                (KeyCode::KeyD, false),
                (KeyCode::Space, false),
                (KeyCode::KeyR, false),
                (KeyCode::KeyA, false),
                (KeyCode::KeyI, false),
                (KeyCode::KeyD, false),
                (KeyCode::Space, false),
                (KeyCode::Digit8, false),
            ]),
            "/motd raid 8"
        );
        assert_eq!(
            compose(&[
                (KeyCode::Slash, false),
                (KeyCode::KeyO, false),
                (KeyCode::Space, false),
                (KeyCode::KeyH, false),
                (KeyCode::KeyI, false),
            ]),
            "/o hi"
        );
    }

    #[test]
    fn compose_keys_cover_every_typed_char() {
        for key in GUILD_COMPOSE_KEYS {
            assert!(
                guild_compose_char_from_key(key, false).is_some(),
                "{key:?} is listed but types nothing"
            );
        }
        assert!(GUILD_COMPOSE_KEYS.contains(&KeyCode::KeyJ));
        // Guild verbs are Ctrl+letter; their letters still type on their own.
        for key in [
            KeyCode::KeyV,
            KeyCode::KeyQ,
            KeyCode::KeyX,
            KeyCode::KeyK,
            KeyCode::KeyP,
            KeyCode::KeyO,
            KeyCode::KeyT,
            KeyCode::KeyD,
        ] {
            assert!(GUILD_COMPOSE_KEYS.contains(&key), "{key:?} must type");
        }
    }

    #[test]
    fn guild_enter_slash_commands_do_not_need_a_target() {
        let mut snap = TickSnapshot::default();
        snap.guild = Some(woc_protocol::GuildSnapshot {
            id: 1,
            name: "Vale Watch".into(),
            rank: "leader".into(),
            ..Default::default()
        });
        assert!(matches!(
            guild_enter_msg("/invite Bob", &snap),
            WsClientMsg::GuildInvite { name } if name == "Bob"
        ));
        assert!(matches!(
            guild_enter_msg("/kick Bob", &snap),
            WsClientMsg::GuildKick { name } if name == "Bob"
        ));
        assert!(matches!(
            guild_enter_msg("/officer Bob", &snap),
            WsClientMsg::GuildSetRank { name, rank } if name == "Bob" && rank == "officer"
        ));
        assert!(matches!(
            guild_enter_msg("/member Bob", &snap),
            WsClientMsg::GuildSetRank { name, rank } if name == "Bob" && rank == "member"
        ));
        assert!(matches!(
            guild_enter_msg("/transfer Bob", &snap),
            WsClientMsg::GuildTransferLeader { name } if name == "Bob"
        ));
        assert!(matches!(
            guild_enter_msg("hello", &snap),
            WsClientMsg::Chat { channel, text } if channel == "guild" && text == "hello"
        ));
    }

    #[test]
    fn guild_enter_create_trims_name_when_not_in_guild() {
        let snap = TickSnapshot::default();
        assert!(matches!(
            guild_enter_msg("  Vale Watch  ", &snap),
            WsClientMsg::GuildCreate { name } if name == "Vale Watch"
        ));
    }

    #[test]
    fn ability_slot_mapping_covers_kit_keys() {
        assert_eq!(AbilitySlot::Primary as u8, 1);
        assert_eq!(AbilitySlot::Slot2 as u8, 2);
        assert_eq!(AbilitySlot::Slot5 as u8, 5);
    }
}
