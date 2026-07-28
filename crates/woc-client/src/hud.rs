//! HUD / bags / quest log UI updates.

use bevy::prelude::*;

use crate::{GameHost, NetStatus, PlayMode};

#[derive(Component)]
pub(crate) struct HudRoot;

#[derive(Component)]
pub(crate) struct HudHpText;

#[derive(Component)]
pub(crate) struct HudXpText;

#[derive(Component)]
pub(crate) struct HudTargetText;

#[derive(Component)]
pub(crate) struct HudToastText;

#[derive(Component)]
pub(crate) struct HudQuestText;

#[derive(Component)]
pub(crate) struct HudBagText;

#[derive(Component)]
pub(crate) struct HudNetText;

#[derive(Resource)]
pub(crate) struct UiFlags {
    pub(crate) show_bags: bool,
    pub(crate) show_quests: bool,
}

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(UiFlags {
        show_bags: false,
        show_quests: false,
    });
}

pub(crate) fn update_hud(
    host: Res<GameHost>,
    ui: Res<UiFlags>,
    mut hp: Query<&mut Text, With<HudHpText>>,
    mut xp: Query<&mut Text, (With<HudXpText>, Without<HudHpText>)>,
    mut target: Query<&mut Text, (With<HudTargetText>, Without<HudHpText>, Without<HudXpText>)>,
    mut quest: Query<
        &mut Text,
        (
            With<HudQuestText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
        ),
    >,
    mut bags: Query<
        &mut Text,
        (
            With<HudBagText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
        ),
    >,
    mut toast: Query<
        &mut Text,
        (
            With<HudToastText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
            Without<HudBagText>,
        ),
    >,
    mut net: Query<
        &mut Text,
        (
            With<HudNetText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
            Without<HudBagText>,
            Without<HudToastText>,
        ),
    >,
) {
    let snap = &host.snapshot;
    if let Some(player) = snap.entities.iter().find(|e| e.id == snap.player_id) {
        if let Ok(mut t) = hp.single_mut() {
            let abil = if snap.ability_name.is_empty() {
                "Ability"
            } else {
                &snap.ability_name
            };
            **t = format!(
                "HP {:.0}/{:.0}   {} {:.0}/{:.0}   [1] {} {}",
                player.hp,
                player.hp_max,
                snap.progress.resource_type,
                player.resource,
                player.resource_max,
                abil,
                if snap.ability_ready { "READY" } else { "CD" }
            );
        }
    } else if let Ok(mut t) = hp.single_mut() {
        **t = "HP --".into();
    }
    if let Ok(mut t) = xp.single_mut() {
        let gear = snap.equipment.main_hand.as_deref().unwrap_or("—");
        **t = format!(
            "Lv {} {}   XP {}/{}   Copper {}   Weapon: {}",
            snap.progress.level,
            snap.progress.class_id,
            snap.progress.xp,
            snap.progress.xp_to_level,
            snap.progress.copper,
            gear
        );
    }
    if let Ok(mut t) = target.single_mut() {
        **t = if let Some(tid) = snap.target_id {
            if let Some(e) = snap.entities.iter().find(|e| e.id == tid) {
                format!(
                    "Target: {}  HP {:.0}/{:.0}{}",
                    e.name,
                    e.hp,
                    e.hp_max,
                    if e.alive { "" } else { " (dead)" }
                )
            } else {
                "Target: none".into()
            }
        } else {
            "Target: none".into()
        };
    }
    if let Ok(mut t) = quest.single_mut() {
        if ui.show_quests {
            if snap.quest_log.is_empty() {
                **t = "Quests: (none — talk to Captain Alden with E)".into();
            } else {
                let lines: Vec<String> = snap
                    .quest_log
                    .iter()
                    .map(|q| format!("{} [{}]", q.quest_id, q.state))
                    .collect();
                **t = format!("Quests: {}", lines.join(" · "));
            }
        } else {
            let active = snap
                .quest_log
                .iter()
                .find(|q| q.state == "active" || q.state == "ready");
            **t = match active {
                Some(q) => format!("Quest: {} [{}] (L list)", q.quest_id, q.state),
                None => "Quest: — (E talk · L list)".into(),
            };
        }
    }
    if let Ok(mut t) = bags.single_mut() {
        if ui.show_bags {
            if snap.inventory.is_empty() {
                **t = "Bags: empty".into();
            } else {
                let items: Vec<String> = snap
                    .inventory
                    .iter()
                    .map(|s| format!("{}×{}", s.count, s.item_id))
                    .collect();
                **t = format!("Bags: {}", items.join(", "));
            }
        } else {
            **t = format!("Bags: {} slots used (B)", snap.inventory.len());
        }
    }
    if let Ok(mut t) = toast.single_mut() {
        **t = host
            .recent_toasts
            .last()
            .map(|(m, _)| m.clone())
            .unwrap_or_default();
    }
    if let Ok(mut t) = net.single_mut() {
        **t = match host.play_mode {
            PlayMode::Offline => "Host: Offline".into(),
            PlayMode::Online => match &host.net_status {
                NetStatus::Idle => "Online: idle".into(),
                NetStatus::Connecting => {
                    format!("Online: connecting… {}", crate::online::ONLINE_WS_URL)
                }
                NetStatus::Connected { player_id } => {
                    format!("Online: connected (player #{player_id})")
                }
                NetStatus::Error(msg) => format!("Online: error — {msg}"),
            },
        };
    }
}

pub(crate) fn toast_fade(time: Res<Time>, mut host: ResMut<GameHost>) {
    let dt = time.delta_secs();
    for (_, life) in &mut host.recent_toasts {
        *life -= dt;
    }
    host.recent_toasts.retain(|(_, life)| *life > 0.0);
}
