//! HUD / bags / quest log / character / vendor / cast / action-bar UI.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_content::{item, talents::talents_for_class, ItemKind};
use woc_protocol::{EntityId, InteractAction, TickSnapshot, VendorSnapshot};

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

#[derive(Debug, Clone, Copy)]
pub(crate) enum ChromePanelKind {
    Talents,
    Bank,
    Mail,
    Market,
}

#[derive(Component)]
pub(crate) struct HudChromePanel(pub(crate) ChromePanelKind);

#[derive(Component)]
pub(crate) struct HudChromeText(pub(crate) ChromePanelKind);

#[derive(Resource)]
pub(crate) struct UiFlags {
    pub(crate) show_bags: bool,
    pub(crate) show_quests: bool,
    pub(crate) show_character: bool,
    pub(crate) show_talents: bool,
    pub(crate) show_bank: bool,
    pub(crate) show_mail: bool,
    pub(crate) show_market: bool,
    pub(crate) show_map: bool,
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
        show_talents: false,
        show_bank: false,
        show_mail: false,
        show_market: false,
        show_map: false,
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

pub(crate) fn first_junk_bag_stack(snap: &TickSnapshot) -> Option<(u8, u32, String)> {
    snap.inventory.iter().find_map(|stack| {
        item(&stack.item_id)
            .map(|def| def.kind == ItemKind::Junk)
            .unwrap_or(false)
            .then(|| (stack.slot, stack.count, stack.item_id.clone()))
    })
}

fn zone_name(snap: &TickSnapshot) -> &str {
    if snap.zone_id.is_empty() {
        "—"
    } else {
        &snap.zone_id
    }
}

fn talent_panel_text(snap: &TickSnapshot) -> String {
    let professions = if snap.professions.is_empty() {
        "none".into()
    } else {
        snap.professions
            .iter()
            .map(|p| format!("{} {}", p.id, p.skill))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut lines = vec![
        "Talents [N]".to_string(),
        format!("Zone: {}   Honor: {}", zone_name(snap), snap.honor),
        format!("Professions: {professions}"),
        format!("Points: {}", snap.talent_points),
    ];
    let mut any = false;
    for def in talents_for_class(&snap.progress.class_id) {
        any = true;
        let rank = snap
            .talents
            .iter()
            .find(|rank| rank.talent_id == def.id)
            .map(|rank| rank.rank)
            .unwrap_or(0);
        lines.push(format!(
            "  {} {}/{} — {}",
            def.id, rank, def.max_rank, def.name
        ));
    }
    if !any {
        for rank in &snap.talents {
            lines.push(format!("  {} rank {}", rank.talent_id, rank.rank));
            any = true;
        }
    }
    if !any {
        lines.push("  (none for current class)".into());
    }
    lines.push("[Enter/Y] Learn first available   [R] Respec".into());
    lines.join("\n")
}

fn bank_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec![
        "Bank [K]".to_string(),
        format!("Zone: {}", zone_name(snap)),
        "Stored:".into(),
    ];
    if snap.bank.is_empty() {
        lines.push("  (empty)".into());
    } else {
        lines.extend(
            snap.bank
                .iter()
                .enumerate()
                .map(|(slot, stack)| format!("  [{slot}] {}×{}", stack.count, stack.item_id)),
        );
    }
    match first_junk_bag_stack(snap) {
        Some((_, count, item_id)) => {
            lines.push(format!("[G] Deposit {count}×{item_id} (first bag junk)"));
        }
        None => lines.push("[G] Deposit first bag junk (none)".into()),
    }
    match snap.bank.first() {
        Some(stack) => lines.push(format!(
            "[H] Withdraw {}×{} (first bank slot)",
            stack.count, stack.item_id
        )),
        None => lines.push("[H] Withdraw first bank slot (empty)".into()),
    }
    lines.join("\n")
}

fn mail_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec!["Mail [I]".to_string(), format!("Zone: {}", zone_name(snap))];
    if snap.mail.is_empty() {
        lines.push("  (inbox empty)".into());
    } else {
        for mail in &snap.mail {
            let mut attachments = Vec::new();
            if mail.copper > 0 {
                attachments.push(format!("{}c", mail.copper));
            }
            if let Some(item_id) = &mail.item_id {
                attachments.push(format!("{}×{item_id}", mail.item_count));
            }
            let suffix = if attachments.is_empty() {
                String::new()
            } else {
                format!(" ({})", attachments.join(" + "))
            };
            lines.push(format!(
                "  #{} {} — {}{}",
                mail.id, mail.from, mail.subject, suffix
            ));
        }
    }
    lines.push("[P] Collect first mail".into());
    lines.join("\n")
}

fn market_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec![
        "Market [U]".to_string(),
        format!(
            "Zone: {}   Copper: {}   Honor: {}",
            zone_name(snap),
            snap.progress.copper,
            snap.honor
        ),
    ];
    if snap.market.is_empty() {
        lines.push("  (no listings)".into());
    } else {
        lines.extend(snap.market.iter().map(|listing| {
            format!(
                "  #{} {}×{} — {}c ({})",
                listing.id, listing.count, listing.item_id, listing.price, listing.seller
            )
        }));
    }
    lines.push("[O] Buy first listing (if affordable)".into());
    lines.join("\n")
}

pub(crate) fn update_chrome_panels(
    host: Res<GameHost>,
    ui: Res<UiFlags>,
    mut panels: Query<(&HudChromePanel, &mut Visibility)>,
    mut texts: Query<(&HudChromeText, &mut Text)>,
) {
    for (panel, mut visibility) in &mut panels {
        let shown = match panel.0 {
            ChromePanelKind::Talents => ui.show_talents,
            ChromePanelKind::Bank => ui.show_bank,
            ChromePanelKind::Mail => ui.show_mail,
            ChromePanelKind::Market => ui.show_market,
        };
        *visibility = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (panel, mut text) in &mut texts {
        **text = match panel.0 {
            ChromePanelKind::Talents => talent_panel_text(&host.snapshot),
            ChromePanelKind::Bank => bank_panel_text(&host.snapshot),
            ChromePanelKind::Mail => mail_panel_text(&host.snapshot),
            ChromePanelKind::Market => market_panel_text(&host.snapshot),
        };
    }
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
                **t = format!("Casting {}… {:.0}%", cast.ability_id, cast.progress * 100.0);
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
                    let label =
                        format!("Buy {} ×{} — {}c", offer.item_id, offer.count, offer.price);
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

#[cfg(test)]
mod tests {
    use super::*;
    use woc_protocol::{
        InvSlotSnapshot, MailSnapshot, MarketListingSnapshot, ProfessionSkillSnapshot,
        TalentRankSnapshot, TickSnapshot,
    };

    fn chrome_snapshot() -> TickSnapshot {
        let mut snap = TickSnapshot::default();
        snap.progress.class_id = "warrior".into();
        snap.progress.copper = 75;
        snap.zone_id = "eastbrook".into();
        snap.honor = 12;
        snap.talent_points = 2;
        snap.talents.push(TalentRankSnapshot {
            talent_id: "warrior_cruelty".into(),
            rank: 1,
        });
        snap.professions.push(ProfessionSkillSnapshot {
            id: "herbalism".into(),
            skill: 18,
        });
        snap.inventory.push(InvSlotSnapshot {
            slot: 0,
            item_id: "wolf_fang".into(),
            count: 3,
        });
        snap.bank.push(InvSlotSnapshot {
            slot: 0,
            item_id: "silverleaf".into(),
            count: 4,
        });
        snap.mail.push(MailSnapshot {
            id: 7,
            from: "Ada".into(),
            subject: "Parcel".into(),
            copper: 9,
            item_id: Some("baked_bread".into()),
            item_count: 2,
        });
        snap.market.push(MarketListingSnapshot {
            id: 11,
            seller: "Grace".into(),
            item_id: "peacebloom".into(),
            count: 5,
            price: 30,
        });
        snap
    }

    #[test]
    fn talent_panel_formats_progression_context_and_help() {
        let text = talent_panel_text(&chrome_snapshot());

        assert!(text.contains("Points: 2"));
        assert!(text.contains("warrior_cruelty 1/5"));
        assert!(text.contains("Zone: eastbrook"));
        assert!(text.contains("Honor: 12"));
        assert!(text.contains("herbalism 18"));
        assert!(text.contains("[Enter/Y] Learn"));
        assert!(text.contains("[R] Respec"));
    }

    #[test]
    fn bank_panel_formats_slots_and_action_targets() {
        let text = bank_panel_text(&chrome_snapshot());

        assert!(text.contains("[0] 4×silverleaf"));
        assert!(text.contains("[G] Deposit 3×wolf_fang"));
        assert!(text.contains("[H] Withdraw 4×silverleaf"));
    }

    #[test]
    fn mail_panel_formats_attachments_and_collect_help() {
        let text = mail_panel_text(&chrome_snapshot());

        assert!(text.contains("#7 Ada — Parcel"));
        assert!(text.contains("9c + 2×baked_bread"));
        assert!(text.contains("[P] Collect first mail"));
    }

    #[test]
    fn market_panel_formats_listings_wallet_and_buy_help() {
        let text = market_panel_text(&chrome_snapshot());

        assert!(text.contains("Copper: 75"));
        assert!(text.contains("#11 5×peacebloom — 30c (Grace)"));
        assert!(text.contains("[O] Buy first listing"));
    }
}
