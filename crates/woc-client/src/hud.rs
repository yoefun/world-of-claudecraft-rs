//! HUD / bags / quest log / character / vendor / cast / action-bar UI.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_content::{can_equip, item, talents::talents_for_class, ItemKind, PlayerClass};
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

pub(crate) fn first_equippable_bag_stack(snap: &TickSnapshot) -> Option<(u8, String)> {
    let class = PlayerClass::parse(&snap.progress.class_id)?;
    let level = snap.progress.level;
    snap.inventory.iter().find_map(|stack| {
        let def = item(&stack.item_id)?;
        can_equip(def, class, level).ok()?;
        Some((stack.slot, stack.item_id.clone()))
    })
}

pub(crate) fn first_consumable_bag_stack(snap: &TickSnapshot) -> Option<(u8, String)> {
    snap.inventory.iter().find_map(|stack| {
        item(&stack.item_id)
            .filter(|def| def.kind == ItemKind::Consumable)
            .map(|_| (stack.slot, stack.item_id.clone()))
    })
}

pub(crate) fn first_listable_bag_stack(snap: &TickSnapshot) -> Option<(u8, u32, String, u32)> {
    snap.inventory.iter().find_map(|stack| {
        let def = item(&stack.item_id)?;
        if matches!(def.kind, ItemKind::Junk | ItemKind::Consumable) {
            let price = def.vendor_sell.max(1).saturating_mul(5);
            Some((
                stack.slot,
                stack.count.min(1).max(1),
                stack.item_id.clone(),
                price,
            ))
        } else {
            None
        }
    })
}

fn zone_name(snap: &TickSnapshot) -> &str {
    if snap.zone_id.is_empty() {
        "—"
    } else {
        &snap.zone_id
    }
}

fn pending_loot_line(snap: &TickSnapshot) -> Option<String> {
    let pending = snap.pending_loot.iter().find(|p| !p.rolled)?;
    let item = if pending.item_id.is_empty() {
        "copper".into()
    } else {
        pending.item_id.clone()
    };
    Some(format!(
        "Loot roll: {item} (+{}c)  [1] Need  [2] Greed  [3] Pass",
        pending.copper
    ))
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
    let class = snap.progress.class_id.as_str();
    let ranks: Vec<(String, u32)> = snap
        .talents
        .iter()
        .map(|t| (t.talent_id.clone(), t.rank))
        .collect();
    let mut lines = vec![
        "Talents [N]".to_string(),
        format!("Zone: {}   Honor: {}", zone_name(snap), snap.honor),
        format!("Professions: {professions}"),
        format!("Points: {}", snap.talent_points),
    ];
    let mut any = false;
    for (idx, def) in talents_for_class(class).enumerate() {
        any = true;
        let rank = ranks
            .iter()
            .find(|(id, _)| id == def.id)
            .map(|(_, r)| *r)
            .unwrap_or(0);
        let unlocked = woc_content::talent_tier_unlocked(class, &ranks, def);
        let status = if !unlocked {
            "LOCKED".to_string()
        } else if rank >= def.max_rank {
            "MAX".to_string()
        } else if snap.talent_points == 0 {
            "no pts".to_string()
        } else {
            "ready".to_string()
        };
        let effect = woc_content::format_talent_effect(def, rank.max(1));
        lines.push(format!(
            "  [{}] T{} {} {}/{} — {} ({}) [{}]",
            idx + 1,
            def.tier,
            def.name,
            rank,
            def.max_rank,
            effect,
            def.id,
            status
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
    lines.push("[1–5] Learn talent   [Y/Enter] first available   [R] Respec".into());
    lines.push(format!("Bonuses: {}", talent_bonus_summary(snap)));
    lines.join("\n")
}

fn talent_bonus_summary(snap: &TickSnapshot) -> String {
    let mut dmg = 0.0f32;
    let mut hp = 0.0f32;
    let mut armor_pct = 0.0f32;
    let mut armor_flat = 0.0f32;
    let mut resource = 0.0f32;
    let mut crit = 0.0f32;
    let mut heal = 0.0f32;
    let mut cleave = 0.0f32;
    for rank in &snap.talents {
        let Some(def) = woc_content::talent(&rank.talent_id) else {
            continue;
        };
        let r = rank.rank as f32;
        match def.effect {
            "damage_pct" => dmg += def.effect_value * r,
            "max_hp_pct" => hp += def.effect_value * r,
            "armor_pct" => armor_pct += def.effect_value * r,
            "armor_flat" => armor_flat += def.effect_value * r,
            "resource_pct" => resource += def.effect_value * r,
            "crit_pct" => crit += def.effect_value * r,
            "heal_pct" => heal += def.effect_value * r,
            "cleave_targets_plus" => cleave += def.effect_value * r,
            _ => {}
        }
    }
    if dmg == 0.0
        && hp == 0.0
        && armor_pct == 0.0
        && armor_flat == 0.0
        && resource == 0.0
        && crit == 0.0
        && heal == 0.0
        && cleave == 0.0
    {
        return "none".into();
    }
    let mut parts = Vec::new();
    if dmg > 0.0 {
        parts.push(format!("+{:.0}% dmg", dmg * 100.0));
    }
    if hp > 0.0 {
        parts.push(format!("+{:.0}% HP", hp * 100.0));
    }
    if armor_pct > 0.0 {
        parts.push(format!("+{:.0}% armor", armor_pct * 100.0));
    }
    if armor_flat > 0.0 {
        parts.push(format!("+{:.0} armor", armor_flat));
    }
    if resource > 0.0 {
        parts.push(format!("+{:.0}% resource", resource * 100.0));
    }
    if crit > 0.0 {
        parts.push(format!("+{:.0}% crit", crit * 100.0));
    }
    if heal > 0.0 {
        parts.push(format!("+{:.0}% heal", heal * 100.0));
    }
    if cleave > 0.0 {
        parts.push(format!("+{:.0} cleave", cleave));
    }
    parts.join(" · ")
}

fn bank_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec![
        "Bank [K]".to_string(),
        format!(
            "Zone: {}   Wallet: {}c   Vault: {}c",
            zone_name(snap),
            snap.progress.copper,
            snap.bank_copper
        ),
        "Stored:".into(),
    ];
    if snap.bank.is_empty() {
        lines.push("  (empty)".into());
    } else {
        lines.extend(snap.bank.iter().enumerate().map(|(i, stack)| {
            format!(
                "  [{}] slot {} — {}×{}",
                i + 1,
                stack.slot,
                stack.count,
                stack.item_id
            )
        }));
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
    lines.push("[1–9] Withdraw numbered bank stack".into());
    lines.push("[J] Deposit all wallet copper · [Y] Withdraw all vault copper".into());
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
            let mine = if listing.mine { " [yours]" } else { "" };
            format!(
                "  #{} {}×{} — {}c ({}){mine}",
                listing.id, listing.count, listing.item_id, listing.price, listing.seller
            )
        }));
    }
    match first_listable_bag_stack(snap) {
        Some((_, _, item_id, price)) => {
            lines.push(format!("[L] List 1×{item_id} for {price}c (+5c fee)"));
        }
        None => lines.push("[L] List first junk/consumable (none)".into()),
    }
    lines.push("[O] Buy first affordable listing (not yours)".into());
    lines.push("[X] Cancel your first listing".into());
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
            **t = format!(
                "HP {:.0}/{:.0}   {} {:.0}/{:.0}{absorb}{stealth}{combo}",
                player.hp,
                player.hp_max,
                snap.progress.resource_type,
                player.resource,
                player.resource_max,
                absorb = if snap.absorb > 0.5 {
                    format!("   absorb {:.0}", snap.absorb)
                } else {
                    String::new()
                },
                stealth = if snap.stealthed { "   STEALTH" } else { "" },
                combo = if snap.combo_points > 0 {
                    let filled = "●".repeat(snap.combo_points as usize);
                    let empty = "○".repeat(5usize.saturating_sub(snap.combo_points as usize));
                    format!("   {filled}{empty}")
                } else {
                    String::new()
                },
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
                let pct = if e.hp_max > 0.0 {
                    (e.hp / e.hp_max * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                let aa = if snap.auto_attack { "  AA" } else { "" };
                format!(
                    "Target: {}  HP {:.0}/{:.0} ({:.0}%){}{}",
                    e.name,
                    e.hp,
                    e.hp_max,
                    pct,
                    aa,
                    if e.alive { "" } else { " (dead)" }
                )
            } else {
                "Target: none".into()
            }
        } else {
            "Target: none (Tab cycle · LMB acquire · Esc clear)".into()
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
                **t = "Bags: empty\n[1-9] Equip/Use slot · [Q] first legal gear · [F] Use consumable · [V] Sell junk (vendor open)"
                    .into();
            } else {
                let mut lines = vec!["Bags [B]".to_string()];
                for s in &snap.inventory {
                    let kind = item(&s.item_id)
                        .map(|d| format!("{:?}", d.kind))
                        .unwrap_or_else(|| "?".into());
                    lines.push(format!(
                        "  [{}] {}×{} ({kind})",
                        s.slot + 1,
                        s.count,
                        s.item_id
                    ));
                }
                lines.push("[1-9] Equip/Use slot · [Q] first legal gear".into());
                lines.push("[F] Use first consumable".into());
                if snap.open_vendor.is_some() {
                    lines.push("[V] Sell first junk to vendor".into());
                }
                **t = lines.join("\n");
            }
        } else if let Some(loot) = pending_loot_line(snap) {
            **t = format!("Bags: {} slots · {loot}", snap.inventory.len());
        } else {
            **t = format!("Bags: {} slots used (B)", snap.inventory.len());
        }
    }
    if let Ok(mut t) = toast.single_mut() {
        if let Some(loot) = pending_loot_line(snap) {
            **t = loot;
        } else {
            **t = host
                .recent_toasts
                .last()
                .map(|(m, _)| m.clone())
                .unwrap_or_default();
        }
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
            "Character\nClass: {class}\nLevel: {}\nXP: {}/{}\nCopper: {}\nTalents: {} pts · {}\nEquipment:\n  Main: {}\n  Off: {}\n  Head: {}\n  Chest: {}\n  Legs: {}\n  Feet: {}\n  Neck: {}\n  Finger: {}\nAP: {:.0}   Armor: {:.0}   SP: {:.0}\n[1-8] Unequip slot",
            snap.progress.level,
            snap.progress.xp,
            snap.progress.xp_to_level,
            snap.progress.copper,
            snap.talent_points,
            talent_bonus_summary(snap),
            eq.main_hand.as_deref().unwrap_or("—"),
            eq.off_hand.as_deref().unwrap_or("—"),
            eq.head.as_deref().unwrap_or("—"),
            eq.chest.as_deref().unwrap_or("—"),
            eq.legs.as_deref().unwrap_or("—"),
            eq.feet.as_deref().unwrap_or("—"),
            eq.neck.as_deref().unwrap_or("—"),
            eq.finger.as_deref().unwrap_or("—"),
            snap.attack_power,
            snap.armor,
            snap.spell_power,
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

    // Action bar: class kit slots 1–5 from snapshot (fallback to primary name).
    if let Ok(mut t) = action.single_mut() {
        **t = format_action_bar(snap);
    }
}

fn format_action_bar(snap: &TickSnapshot) -> String {
    if snap.ability_bar.is_empty() {
        let name = if snap.ability_name.is_empty() {
            "Ability"
        } else {
            snap.ability_name.as_str()
        };
        let ready = if snap.ability_ready { "READY" } else { "CD" };
        return format!("[1] {name} {ready}   [2] —   [3] —   [4] —   [5] —");
    }
    let mut parts = Vec::new();
    for slot in 1u8..=5 {
        if let Some(entry) = snap.ability_bar.iter().find(|e| e.slot == slot) {
            let status = if !entry.known {
                "locked".to_string()
            } else if entry.ready {
                "READY".to_string()
            } else if entry.cooldown > 0.0 {
                format!("{:.1}s", entry.cooldown)
            } else if snap.gcd > 0.0 {
                "GCD".to_string()
            } else {
                "…".to_string()
            };
            parts.push(format!("[{slot}] {} {status}", entry.name));
        } else {
            parts.push(format!("[{slot}] —"));
        }
    }
    let auras = if snap.auras.is_empty() {
        String::new()
    } else {
        let list = snap
            .auras
            .iter()
            .map(|a| {
                if a.stacks > 1 {
                    format!("{}×{} {:.0}s", a.stacks, a.id, a.remaining)
                } else {
                    format!("{} {:.0}s", a.id, a.remaining)
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        format!("   | Auras: {list}")
    };
    format!(
        "{}{auras}{hint}",
        parts.join("   "),
        hint = class_interact_hint(snap)
    )
}

fn class_interact_hint(snap: &TickSnapshot) -> &'static str {
    match snap.progress.class_id.as_str() {
        "rogue" => {
            if snap.stealthed {
                "   [Z] STEALTH"
            } else {
                "   [Z] Stealth"
            }
        }
        "warrior" => "   [F] Stance",
        "shaman" | "druid" => "   [F] Form",
        _ => "",
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
            mine: false,
        });
        snap
    }

    #[test]
    fn talent_panel_formats_progression_context_and_help() {
        let text = talent_panel_text(&chrome_snapshot());

        assert!(text.contains("Points: 2"));
        assert!(text.contains("Cruelty"));
        assert!(text.contains("[1]"));
        assert!(text.contains("Zone: eastbrook"));
        assert!(text.contains("Honor: 12"));
        assert!(text.contains("herbalism 18"));
        assert!(text.contains("[1–5] Learn"));
        assert!(text.contains("[R] Respec"));
        assert!(text.contains("Bonuses:"));
        assert!(text.contains("+5% dmg") || text.contains("dmg"));
    }

    #[test]
    fn bank_panel_formats_slots_and_action_targets() {
        let text = bank_panel_text(&chrome_snapshot());

        assert!(text.contains("Vault:"));
        assert!(text.contains("slot 0 — 4×silverleaf") || text.contains("4×silverleaf"));
        assert!(text.contains("[G] Deposit 3×wolf_fang"));
        assert!(text.contains("[H] Withdraw 4×silverleaf"));
        assert!(text.contains("[J] Deposit all wallet copper"));
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
        assert!(text.contains("[O] Buy first affordable listing"));
        assert!(text.contains("[L] List"));
        assert!(text.contains("[X] Cancel"));
    }

    #[test]
    fn action_bar_formats_kit_and_auras() {
        let mut snap = TickSnapshot::default();
        snap.ability_bar = vec![
            woc_protocol::AbilityBarSlot {
                slot: 1,
                ability_id: "heroic_strike".into(),
                name: "Heroic Strike".into(),
                known: true,
                ready: true,
                cooldown: 0.0,
            },
            woc_protocol::AbilityBarSlot {
                slot: 2,
                ability_id: "cleave".into(),
                name: "Cleave".into(),
                known: false,
                ready: false,
                cooldown: 0.0,
            },
            woc_protocol::AbilityBarSlot {
                slot: 3,
                ability_id: "execute".into(),
                name: "Execute".into(),
                known: false,
                ready: false,
                cooldown: 0.0,
            },
        ];
        snap.auras.push(woc_protocol::AuraSnapshot {
            id: "rend".into(),
            remaining: 6.0,
            stacks: 1,
        });
        let text = format_action_bar(&snap);
        assert!(text.contains("[1] Heroic Strike READY"));
        assert!(text.contains("[2] Cleave locked"));
        assert!(text.contains("Auras: rend 6s"));
        assert!(!text.contains("[Z] Stealth"));
    }

    #[test]
    fn rogue_action_bar_hints_stealth_key() {
        let mut snap = chrome_snapshot();
        snap.progress.class_id = "rogue".into();
        snap.ability_bar = vec![woc_protocol::AbilityBarSlot {
            slot: 1,
            ability_id: "sinister_strike".into(),
            name: "Sinister Strike".into(),
            known: true,
            ready: true,
            cooldown: 0.0,
        }];
        let text = format_action_bar(&snap);
        assert!(text.contains("[Z] Stealth"));
        snap.stealthed = true;
        let stealthed = format_action_bar(&snap);
        assert!(stealthed.contains("[Z] STEALTH"));
    }

    #[test]
    fn warrior_and_druid_action_bar_hint_f_key() {
        let mut snap = chrome_snapshot();
        snap.progress.class_id = "warrior".into();
        assert!(format_action_bar(&snap).contains("[F] Stance"));
        snap.progress.class_id = "druid".into();
        assert!(format_action_bar(&snap).contains("[F] Form"));
        snap.progress.class_id = "shaman".into();
        assert!(format_action_bar(&snap).contains("[F] Form"));
        snap.progress.class_id = "mage".into();
        let mage = format_action_bar(&snap);
        assert!(!mage.contains("[F] Stance"));
        assert!(!mage.contains("[F] Form"));
    }
}
