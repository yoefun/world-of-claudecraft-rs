//! HUD / bags / quest log / character / vendor / cast / action-bar UI.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_protocol::{EntityId, InteractAction, VendorSnapshot};

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

#[derive(Component)]
pub(crate) struct HudCharPanel;

#[derive(Component)]
pub(crate) struct HudCharText;

#[derive(Component)]
pub(crate) struct HudVendorPanel;

#[derive(Component)]
pub(crate) struct HudVendorTitle;

#[derive(Component)]
pub(crate) struct HudVendorOffers;

#[derive(Component)]
pub(crate) struct VendorBuyButton {
    pub(crate) npc_id: EntityId,
    pub(crate) item_id: String,
    pub(crate) count: u32,
}

#[derive(Component)]
pub(crate) struct HudCastPanel;

#[derive(Component)]
pub(crate) struct HudCastText;

#[derive(Component)]
pub(crate) struct HudCastFill;

#[derive(Component)]
pub(crate) struct HudActionBarText;

#[derive(Resource)]
pub(crate) struct UiFlags {
    pub(crate) show_bags: bool,
    pub(crate) show_quests: bool,
    pub(crate) show_character: bool,
}

#[derive(Resource, Default)]
pub(crate) struct VendorUiCache {
    key: Option<String>,
}

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(UiFlags {
        show_bags: false,
        show_quests: false,
        show_character: false,
    })
    .init_resource::<VendorUiCache>();
}

fn vendor_cache_key(v: &VendorSnapshot) -> String {
    let mut key = format!("{}|{}", v.npc_id, v.npc_name);
    for o in &v.stock {
        key.push_str(&format!("|{}:{}:{}", o.item_id, o.count, o.price));
    }
    key
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
    mut char_text: Query<
        &mut Text,
        (
            With<HudCharText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
            Without<HudBagText>,
            Without<HudToastText>,
            Without<HudNetText>,
        ),
    >,
    mut char_panel: Query<&mut Visibility, With<HudCharPanel>>,
    mut cast_text: Query<
        &mut Text,
        (
            With<HudCastText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
            Without<HudBagText>,
            Without<HudToastText>,
            Without<HudNetText>,
            Without<HudCharText>,
        ),
    >,
    mut cast_panel: Query<&mut Visibility, (With<HudCastPanel>, Without<HudCharPanel>)>,
    mut cast_fill: Query<&mut Node, With<HudCastFill>>,
    mut action: Query<
        &mut Text,
        (
            With<HudActionBarText>,
            Without<HudHpText>,
            Without<HudXpText>,
            Without<HudTargetText>,
            Without<HudQuestText>,
            Without<HudBagText>,
            Without<HudToastText>,
            Without<HudNetText>,
            Without<HudCharText>,
            Without<HudCastText>,
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

    // Character sheet (C)
    if let Ok(mut vis) = char_panel.single_mut() {
        *vis = if ui.show_character {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut t) = char_text.single_mut() {
        let eq = &snap.equipment;
        let class = if snap.progress.class_id.is_empty() {
            "—"
        } else {
            snap.progress.class_id.as_str()
        };
        **t = format!(
            "Character\nClass: {class}\nLevel: {}\nXP: {}/{}\nCopper: {}\nEquipment:\n  Main: {}\n  Off: {}\n  Head: {}\n  Chest: {}\n  Legs: {}\n  Feet: {}",
            snap.progress.level,
            snap.progress.xp,
            snap.progress.xp_to_level,
            snap.progress.copper,
            eq.main_hand.as_deref().unwrap_or("—"),
            eq.off_hand.as_deref().unwrap_or("—"),
            eq.head.as_deref().unwrap_or("—"),
            eq.chest.as_deref().unwrap_or("—"),
            eq.legs.as_deref().unwrap_or("—"),
            eq.feet.as_deref().unwrap_or("—"),
        );
    }

    // Cast bar
    match &snap.cast {
        Some(cast) => {
            if let Ok(mut vis) = cast_panel.single_mut() {
                *vis = Visibility::Visible;
            }
            if let Ok(mut t) = cast_text.single_mut() {
                **t = format!(
                    "Casting {}… {:.0}%",
                    cast.ability_id,
                    cast.progress * 100.0
                );
            }
            if let Ok(mut node) = cast_fill.single_mut() {
                node.width = Val::Percent((cast.progress * 100.0).clamp(0.0, 100.0));
            }
        }
        None => {
            if let Ok(mut vis) = cast_panel.single_mut() {
                *vis = Visibility::Hidden;
            }
            if let Ok(mut t) = cast_text.single_mut() {
                **t = String::new();
            }
            if let Ok(mut node) = cast_fill.single_mut() {
                node.width = Val::Percent(0.0);
            }
        }
    }

    // Action bar: Primary + slots 2–5 labels (kits later)
    if let Ok(mut t) = action.single_mut() {
        let name = if snap.ability_name.is_empty() {
            "Ability"
        } else {
            snap.ability_name.as_str()
        };
        let ready = if snap.ability_ready { "READY" } else { "CD" };
        **t = format!("[1] {name} {ready}   [2] —   [3] —   [4] —   [5] —");
    }
}

/// Show vendor panel when `open_vendor` is set; rebuild buy buttons as stock changes.
pub(crate) fn sync_vendor_panel(
    mut commands: Commands,
    host: Res<GameHost>,
    mut cache: ResMut<VendorUiCache>,
    mut panel: Query<&mut Visibility, With<HudVendorPanel>>,
    mut title: Query<&mut Text, With<HudVendorTitle>>,
    offers_root: Query<Entity, With<HudVendorOffers>>,
) {
    match &host.snapshot.open_vendor {
        Some(vendor) => {
            if let Ok(mut vis) = panel.single_mut() {
                *vis = Visibility::Visible;
            }
            if let Ok(mut t) = title.single_mut() {
                **t = format!("Vendor: {}", vendor.npc_name);
            }
            let key = vendor_cache_key(vendor);
            if cache.key.as_ref() == Some(&key) {
                return;
            }
            cache.key = Some(key);
            let Ok(offers_e) = offers_root.single() else {
                return;
            };
            commands.entity(offers_e).despawn_related::<Children>();
            let npc_id = vendor.npc_id;
            let stock = vendor.stock.clone();
            let empty = stock.is_empty();
            commands.entity(offers_e).with_children(|parent| {
                for offer in stock {
                    let label = format!(
                        "Buy {} ×{} — {}c",
                        offer.item_id, offer.count, offer.price
                    );
                    parent
                        .spawn((
                            Button,
                            VendorBuyButton {
                                npc_id,
                                item_id: offer.item_id,
                                count: offer.count.max(1),
                            },
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.12, 0.22, 0.16, 0.92)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(label),
                                TextFont::from_font_size(15.0),
                                TextColor(Color::srgb(0.9, 0.95, 0.85)),
                            ));
                        });
                }
                if empty {
                    parent.spawn((
                        Text::new("(no stock)"),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.7, 0.75, 0.7)),
                    ));
                }
            });
        }
        None => {
            cache.key = None;
            if let Ok(mut vis) = panel.single_mut() {
                *vis = Visibility::Hidden;
            }
            if let Ok(offers_e) = offers_root.single() {
                commands.entity(offers_e).despawn_related::<Children>();
            }
        }
    }
}

pub(crate) fn vendor_buy_clicks(
    interactions: Query<(&Interaction, &VendorBuyButton), Changed<Interaction>>,
    mut host: ResMut<GameHost>,
) {
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        host.interact(
            btn.npc_id,
            InteractAction::Buy {
                item_id: btn.item_id.clone(),
                count: btn.count,
            },
        );
        host.recent_toasts
            .push((format!("Buying {} ×{}", btn.item_id, btn.count), 1.5));
    }
}

/// Release look-grab while a vendor is open so Buy buttons are clickable.
pub(crate) fn vendor_ungrab_cursor(
    mut host: ResMut<GameHost>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if host.snapshot.open_vendor.is_none() || !host.cursor_grabbed {
        return;
    }
    host.cursor_grabbed = false;
    if let Ok(mut window) = windows.single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
}

pub(crate) fn toast_fade(time: Res<Time>, mut host: ResMut<GameHost>) {
    let dt = time.delta_secs();
    for (_, life) in &mut host.recent_toasts {
        *life -= dt;
    }
    host.recent_toasts.retain(|(_, life)| *life > 0.0);
}
