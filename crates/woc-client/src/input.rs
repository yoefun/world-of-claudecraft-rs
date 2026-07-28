//! Intent collection, camera look, interact keys.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, InteractAction, PlayerIntent};

use crate::hud::UiFlags;
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
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        intent.ability = Some(AbilitySlot::Primary);
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

    let is_wilkes = host
        .snapshot
        .entities
        .iter()
        .find(|e| e.id == nid)
        .and_then(|e| e.template_id.as_deref())
        == Some("trader_wilkes");
    if is_wilkes && host.snapshot.progress.copper >= 12 {
        host.interact(
            nid,
            InteractAction::Buy {
                item_id: "travelers_ration".into(),
                count: 1,
            },
        );
    }
}
