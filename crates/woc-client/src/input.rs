//! Intent collection, camera look, interact keys.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_content::talents::talents_for_class;
use woc_protocol::{AbilitySlot, EntityId, EntityKind, InteractAction, PlayerIntent, TickSnapshot};

use crate::hud::{first_junk_bag_stack, UiFlags};
use crate::GameHost;

pub(crate) fn grab_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut host: ResMut<GameHost>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
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

pub(crate) fn collect_intent(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut host: ResMut<GameHost>,
    ui: Res<UiFlags>,
) {
    let mut intent = PlayerIntent {
        facing: host.look_yaw,
        ..default()
    };
    let mut mx = 0.0;
    let mut mz = 0.0;
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
    intent.move_x = mx;
    intent.move_z = mz;
    if keys.just_pressed(KeyCode::Space) || keys.pressed(KeyCode::Space) {
        // Held Space: jump / swim hop / fly ascend (matches upstream MoveInput.jump).
        intent.jump = true;
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        intent.descend = true;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        intent.fly_toggle = true;
    }
    // When the talent panel is open, digit keys spend points instead of casting.
    if !ui.show_talents {
        if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
            intent.ability = Some(AbilitySlot::Primary);
        }
        // Slots 2–5 reserved for ability kits (no-op until protocol/sim expose them).
        let _ = (
            keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2),
            keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3),
            keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4),
            keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::Numpad5),
        );
    }
    if mouse.just_pressed(MouseButton::Left) || keys.pressed(KeyCode::KeyF) {
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
            if sim.player().map(|p| p.auto_attack).unwrap_or(false) {
                intent.attack = true;
                intent.target_id = intent
                    .target_id
                    .or_else(|| sim.player().and_then(|p| p.target));
            }
        }
    } else if host.local_auto_attack {
        intent.attack = true;
        intent.target_id = intent.target_id.or(host.snapshot.target_id);
    }
    host.pending_intent = intent;
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

pub(crate) fn handle_interact_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut host: ResMut<GameHost>,
    mut ui: ResMut<UiFlags>,
) {
    if keys.just_pressed(KeyCode::KeyB) {
        ui.show_bags = !ui.show_bags;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        ui.show_quests = !ui.show_quests;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        ui.show_character = !ui.show_character;
        if ui.show_character {
            ui.show_talents = false;
            ui.show_bank = false;
            ui.show_mail = false;
            ui.show_market = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyN) {
        ui.show_talents = !ui.show_talents;
        if ui.show_talents {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyK) {
        ui.show_bank = !ui.show_bank;
        if ui.show_bank {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyI) {
        ui.show_mail = !ui.show_mail;
        if ui.show_mail {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyM) {
        ui.show_map = !ui.show_map;
        if ui.show_map {
            ui.show_character = false;
            ui.show_talents = false;
            ui.show_bank = false;
            ui.show_mail = false;
            ui.show_market = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyU) {
        ui.show_market = !ui.show_market;
        if ui.show_market {
            ui.show_character = false;
            ui.show_map = false;
        }
    }
    if keys.just_pressed(KeyCode::Escape) && ui.show_map {
        ui.show_map = false;
    }

    let player_id = host.snapshot.player_id;
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

    // Pet toggle (hunter/warlock): T — when mail panel is closed so P stays mail-collect.
    if !ui.show_mail && keys.just_pressed(KeyCode::KeyT) {
        if local_pet_alive(&host.snapshot) {
            host.interact(player_id, InteractAction::DismissPet);
            host.recent_toasts.push(("Dismissing pet…".into(), 1.5));
        } else {
            host.interact(player_id, InteractAction::SummonPet);
            host.recent_toasts.push(("Summoning pet…".into(), 1.5));
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
    if ui.show_mail && keys.just_pressed(KeyCode::KeyP) {
        if let Some(mail) = host.snapshot.mail.first() {
            let mail_id = mail.id;
            host.interact(player_id, InteractAction::MailCollect { mail_id });
            host.recent_toasts
                .push((format!("Collecting mail #{mail_id}."), 2.0));
        } else {
            host.recent_toasts.push(("Inbox is empty.".into(), 2.0));
        }
    }
    if ui.show_market && keys.just_pressed(KeyCode::KeyO) {
        if let Some(listing) = host.snapshot.market.first().cloned() {
            if listing.price <= host.snapshot.progress.copper {
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
                    .push(("Not enough copper for first listing.".into(), 2.0));
            }
        } else {
            host.recent_toasts.push(("No market listings.".into(), 2.0));
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
    let mut best: Option<(EntityId, f32, bool)> = None;
    for e in &host.snapshot.entities {
        if e.kind != EntityKind::Npc || !e.alive {
            continue;
        }
        let dx = e.x - player.x;
        let dz = e.z - player.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d < 5.0 && best.map(|(_, bd, _)| d < bd).unwrap_or(true) {
            best = Some((e.id, d, e.template_id.as_deref() == Some("captain_alden")));
        }
    }
    let Some((nid, _, is_alden)) = best else {
        host.recent_toasts.push(("No NPC nearby.".into(), 2.0));
        return;
    };

    host.interact(nid, InteractAction::Talk);

    if is_alden {
        let log = host.snapshot.quest_log.clone();
        let has_wolves = log.iter().any(|q| q.quest_id == "wolves_at_the_gate");
        let wolves_ready = log
            .iter()
            .any(|q| q.quest_id == "wolves_at_the_gate" && q.state == "ready");
        if wolves_ready {
            host.interact(
                nid,
                InteractAction::TurnInQuest {
                    quest_id: "wolves_at_the_gate".into(),
                },
            );
        } else if !has_wolves {
            host.interact(
                nid,
                InteractAction::AcceptQuest {
                    quest_id: "wolves_at_the_gate".into(),
                },
            );
        }
        let has_boar = log.iter().any(|q| q.quest_id == "boar_tusks");
        let boar_ready = log
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && q.state == "ready");
        if boar_ready {
            host.interact(
                nid,
                InteractAction::TurnInQuest {
                    quest_id: "boar_tusks".into(),
                },
            );
        } else if !has_boar && has_wolves {
            host.interact(
                nid,
                InteractAction::AcceptQuest {
                    quest_id: "boar_tusks".into(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::{InvSlotSnapshot, TalentRankSnapshot, TickSnapshot};

    #[test]
    fn first_available_talent_uses_class_and_skips_max_rank() {
        let mut snap = TickSnapshot::default();
        snap.progress.class_id = "warrior".into();
        for id in ["warrior_cruelty", "warrior_toughness", "warrior_vitality"] {
            snap.talents.push(TalentRankSnapshot {
                talent_id: id.into(),
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
        });
        snap.inventory.push(InvSlotSnapshot {
            slot: 1,
            item_id: "wolf_fang".into(),
            count: 3,
        });

        assert_eq!(
            first_junk_bag_stack(&snap),
            Some((1, 3, "wolf_fang".into()))
        );
    }
}
