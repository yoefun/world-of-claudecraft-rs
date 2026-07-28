//! Intent collection, camera look, interact keys.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_protocol::{AbilitySlot, EntityId, EntityKind, InteractAction, PlayerIntent};

use crate::hud::UiFlags;
use crate::OfflineHost;

pub(crate) fn grab_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut host: ResMut<OfflineHost>,
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

pub(crate) fn camera_look(mut motion: EventReader<MouseMotion>, mut host: ResMut<OfflineHost>) {
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
    mut host: ResMut<OfflineHost>,
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
        if let Some(p) = host.sim.player() {
            let (px, pz) = (p.x, p.z);
            let mut best: Option<(EntityId, f32)> = None;
            for e in &host.sim.entities {
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
    if host.sim.player().map(|p| p.auto_attack).unwrap_or(false) {
        intent.attack = true;
        intent.target_id = intent
            .target_id
            .or_else(|| host.sim.player().and_then(|p| p.target));
    }
    host.pending_intent = intent;
}

pub(crate) fn handle_interact_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut host: ResMut<OfflineHost>,
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
    let Some(player) = host.sim.player().cloned() else {
        return;
    };
    // Prefer nearest NPC; if vendor open and Digit2.. accept first available quest via talk flow.
    let mut best: Option<(EntityId, f32, bool)> = None;
    for e in &host.sim.entities {
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

    // Talk first.
    host.sim.interact(nid, InteractAction::Talk);

    // Auto-accept / turn-in Alden quests if available.
    if is_alden {
        let log = host
            .sim
            .player()
            .map(|p| p.quest_log.clone())
            .unwrap_or_default();
        let has_wolves = log.iter().any(|q| q.quest_id == "wolves_at_the_gate");
        let wolves_ready = log.iter().any(|q| {
            q.quest_id == "wolves_at_the_gate" && matches!(q.state, woc_sim::QuestState::Ready)
        });
        if wolves_ready {
            host.sim.interact(
                nid,
                InteractAction::TurnInQuest {
                    quest_id: "wolves_at_the_gate".into(),
                },
            );
        } else if !has_wolves {
            host.sim.interact(
                nid,
                InteractAction::AcceptQuest {
                    quest_id: "wolves_at_the_gate".into(),
                },
            );
        }
        let has_boar = log.iter().any(|q| q.quest_id == "boar_tusks");
        let boar_ready = log
            .iter()
            .any(|q| q.quest_id == "boar_tusks" && matches!(q.state, woc_sim::QuestState::Ready));
        if boar_ready {
            host.sim.interact(
                nid,
                InteractAction::TurnInQuest {
                    quest_id: "boar_tusks".into(),
                },
            );
        } else if !has_boar && has_wolves {
            host.sim.interact(
                nid,
                InteractAction::AcceptQuest {
                    quest_id: "boar_tusks".into(),
                },
            );
        }
    }

    // Buy ration from vendor if talking to Wilkes and copper allows.
    if host
        .sim
        .entities
        .iter()
        .find(|e| e.id == nid)
        .and_then(|e| e.template_id.as_deref())
        == Some("trader_wilkes")
        && host.sim.copper() >= 12
    {
        host.sim.interact(
            nid,
            InteractAction::Buy {
                item_id: "travelers_ration".into(),
                count: 1,
            },
        );
    }
}
