//! HUD / bags / quest log / character / vendor / cast / action-bar UI.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use woc_content::{
    can_equip, enchant, item, quest, talents::talents_for_class, ItemKind, ItemQuality,
    PlayerClass, QuestObjective,
};
use woc_protocol::{EntityId, InteractAction, QuestLogEntry, TickSnapshot, VendorOfferSnapshot};

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
pub(crate) struct HudPartyFrames;

#[derive(Component)]
pub(crate) struct HudPartyPanel;

#[derive(Component)]
pub(crate) struct HudPartyText;

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
pub(crate) struct NpcSessionButton {
    pub(crate) npc_id: EntityId,
    pub(crate) action: InteractAction,
    pub(crate) toast: String,
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
    Guild,
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
    /// Mail recipient buffer, seeded from the current target when mail opens.
    pub(crate) mail_to: String,
    /// True while the mail recipient buffer has keyboard focus (Enter toggles).
    pub(crate) mail_compose: bool,
    pub(crate) show_party: bool,
    pub(crate) show_guild: bool,
    pub(crate) guild_compose: String,
    pub(crate) market_filter: String,
    pub(crate) market_page: usize,
    pub(crate) market_duration_hours: u32,
    pub(crate) market_searching: bool,
}

impl Default for UiFlags {
    fn default() -> Self {
        Self {
            show_bags: false,
            show_quests: false,
            show_character: false,
            show_talents: false,
            show_bank: false,
            show_mail: false,
            show_market: false,
            show_map: false,
            mail_to: String::new(),
            mail_compose: false,
            show_party: false,
            show_guild: false,
            guild_compose: String::new(),
            market_filter: String::new(),
            market_page: 0,
            market_duration_hours: 12,
            market_searching: false,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct VendorUiCache {
    key: Option<String>,
}

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(UiFlags::default())
        .init_resource::<VendorUiCache>();
}

fn npc_session_stock(snap: &TickSnapshot) -> Vec<VendorOfferSnapshot> {
    if let Some(npc) = &snap.open_npc {
        if !npc.stock.is_empty() {
            return npc.stock.clone();
        }
    }
    snap.open_vendor
        .as_ref()
        .map(|v| v.stock.clone())
        .unwrap_or_default()
}

fn npc_session_cache_key(snap: &TickSnapshot) -> Option<String> {
    let Some(npc) = &snap.open_npc else {
        let vendor = snap.open_vendor.as_ref()?;
        let mut key = format!("vendor|{}|{}", vendor.npc_id, vendor.npc_name);
        for o in &vendor.stock {
            key.push_str(&format!("|{}:{}:{}", o.item_id, o.count, o.price));
        }
        return Some(key);
    };

    let mut key = format!(
        "npc|{}|{}|{}|{}|{}",
        npc.npc_id, npc.npc_name, npc.can_repair, npc.repair_cost, npc.can_bind
    );
    for service in &npc.services {
        key.push_str(&format!("|svc:{service}"));
    }
    for profession in &npc.train_professions {
        key.push_str(&format!("|train:{profession}"));
    }
    for o in npc_session_stock(snap) {
        key.push_str(&format!("|{}:{}:{}", o.item_id, o.count, o.price));
    }
    for row in &npc.buyback {
        key.push_str(&format!(
            "|buyback:{}:{}:{}:{}",
            row.slot, row.item_id, row.count, row.price
        ));
    }
    Some(key)
}

pub(crate) fn first_junk_bag_stack(snap: &TickSnapshot) -> Option<(u8, u32, String)> {
    snap.inventory.iter().find_map(|stack| {
        item(&stack.item_id)
            .map(|def| def.kind == ItemKind::Junk)
            .unwrap_or(false)
            .then(|| (stack.slot, stack.count, stack.item_id.clone()))
    })
}

/// First bag stack the bank will accept: any kind except `ItemKind::Quest`.
pub(crate) fn first_bankable_bag_stack(snap: &TickSnapshot) -> Option<(u8, u32, String)> {
    snap.inventory.iter().find_map(|stack| {
        item(&stack.item_id)
            .map(|def| def.kind != ItemKind::Quest)
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
        if matches!(def.kind, ItemKind::Quest) || stack.bound {
            return None;
        }
        let price = def.vendor_sell.max(1).saturating_mul(5);
        Some((
            stack.slot,
            stack.count.min(1).max(1),
            stack.item_id.clone(),
            price,
        ))
    })
}

pub(crate) const MARKET_PAGE_SIZE: usize = 8;

pub(crate) fn filtered_market<'a>(
    snap: &'a TickSnapshot,
    filter: &str,
) -> Vec<&'a woc_protocol::MarketListingSnapshot> {
    let needle = filter.trim().to_ascii_lowercase();
    snap.market
        .iter()
        .filter(|listing| {
            if needle.is_empty() {
                return true;
            }
            let id = listing.item_id.to_ascii_lowercase();
            let name = item(&listing.item_id)
                .map(|d| d.name.to_ascii_lowercase())
                .unwrap_or_default();
            id.contains(&needle) || name.contains(&needle)
        })
        .collect()
}

pub(crate) fn listing_min_bid(listing: &woc_protocol::MarketListingSnapshot) -> Option<u32> {
    if listing.start_bid == 0 && listing.current_bid == 0 {
        return None;
    }
    if listing.bidder.is_none() {
        return Some(listing.start_bid.max(1));
    }
    Some(
        listing
            .current_bid
            .saturating_add((listing.current_bid / 20).max(1)),
    )
}

pub(crate) fn cycle_duration_hours(hours: u32, next: bool) -> u32 {
    match (hours, next) {
        (12, true) | (0, true) => 24,
        (24, true) => 48,
        (48, true) => 12,
        (12, false) | (0, false) => 48,
        (24, false) => 12,
        (48, false) => 24,
        (_, true) => 24,
        (_, false) => 12,
    }
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

pub(crate) fn npc_session_help(snap: &TickSnapshot) -> String {
    let mut parts = Vec::new();
    if snap
        .open_npc
        .as_ref()
        .map(|npc| npc.can_repair)
        .unwrap_or(false)
    {
        parts.push("[R] Repair".into());
    }
    parts.push("[H] Hearthstone".into());
    if snap
        .open_npc
        .as_ref()
        .map(|npc| npc.discount_pct > 0)
        .unwrap_or(false)
    {
        let pct = snap.open_npc.as_ref().map(|n| n.discount_pct).unwrap_or(0);
        parts.push(format!("[{pct}% off]"));
    }
    if snap
        .open_npc
        .as_ref()
        .map(|npc| !npc.train_professions.is_empty())
        .unwrap_or(false)
    {
        parts.push("[T] Train".into());
    }
    if snap
        .open_npc
        .as_ref()
        .map(|npc| npc.can_auction)
        .unwrap_or(false)
    {
        parts.push("[U] Auction".into());
    }
    if snap
        .open_npc
        .as_ref()
        .map(|npc| npc.can_bank)
        .unwrap_or(false)
    {
        parts.push("[K] Bank".into());
    }
    if snap
        .open_npc
        .as_ref()
        .map(|npc| npc.can_mail)
        .unwrap_or(false)
    {
        parts.push("[I] Mail".into());
    }
    parts.join("   ")
}

fn gear_durability_text(item_id: &str, durability: Option<u32>) -> Option<String> {
    let def = item(item_id)?;
    if def.max_durability == 0 {
        return None;
    }
    let dur = durability.unwrap_or(def.max_durability);
    let broken = if dur == 0 { " BROKEN" } else { "" };
    Some(format!("{} {dur}/{}{broken}", def.name, def.max_durability))
}

fn bag_stack_label(item_id: &str, count: u32, durability: Option<u32>) -> String {
    if let Some(label) = gear_durability_text(item_id, durability) {
        return label;
    }
    format!("{count}×{item_id}")
}

fn quality_prefix(instance: Option<&str>, item_id: Option<&str>) -> &'static str {
    let q = instance
        .and_then(ItemQuality::parse)
        .or_else(|| item_id.and_then(item).map(|d| d.quality));
    match q {
        Some(ItemQuality::Uncommon) => "Uncommon ",
        Some(ItemQuality::Rare) => "Rare ",
        Some(ItemQuality::Poor) => "Poor ",
        _ => "",
    }
}

fn equipment_label(
    item_id: Option<&str>,
    durability: Option<u32>,
    instance_quality: Option<&str>,
) -> String {
    match item_id {
        Some(id) => {
            let base = gear_durability_text(id, durability)
                .or_else(|| item(id).map(|d| d.name.to_string()))
                .unwrap_or_else(|| id.to_string());
            format!("{}{base}", quality_prefix(instance_quality, Some(id)))
        }
        None => "—".into(),
    }
}

fn weapon_label(
    item_id: Option<&str>,
    durability: Option<u32>,
    instance_quality: Option<&str>,
    enchant_id: Option<&str>,
) -> String {
    let mut label = equipment_label(item_id, durability, instance_quality);
    if let Some(eid) = enchant_id {
        if let Some(def) = enchant(eid) {
            label = format!("{label} [{}]", def.name);
        }
    }
    label
}

fn standing_display(standing: &str) -> &str {
    match standing {
        "hated" => "Hated",
        "hostile" => "Hostile",
        "unfriendly" => "Unfriendly",
        "neutral" => "Neutral",
        "friendly" => "Friendly",
        "honored" => "Honored",
        "revered" => "Revered",
        "exalted" => "Exalted",
        other => other,
    }
}

fn reputation_block(snap: &TickSnapshot) -> String {
    if snap.reputation.is_empty() {
        return "Reputation: —".into();
    }
    let mut lines = vec!["Reputation:".to_string()];
    for row in &snap.reputation {
        lines.push(format!(
            "  {}  {} ({})",
            row.name,
            standing_display(&row.standing),
            row.value
        ));
    }
    lines.join("\n")
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

/// Bank/mail line label: gear shows durability, then enchant name when present.
fn instance_label(
    item_id: &str,
    count: u32,
    durability: Option<u32>,
    enchant_id: Option<&str>,
) -> String {
    let mut label = bag_stack_label(item_id, count, durability);
    if let Some(eid) = enchant_id {
        if let Some(def) = enchant(eid) {
            label = format!("{label} [{}]", def.name);
        }
    }
    label
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
                "  [{}] slot {} — {}",
                i + 1,
                stack.slot,
                instance_label(
                    &stack.item_id,
                    stack.count,
                    stack.durability,
                    stack.enchant_id.as_deref()
                )
            )
        }));
    }
    match first_bankable_bag_stack(snap) {
        Some((_, count, item_id)) => {
            lines.push(format!("[G] Deposit {count}×{item_id} (first bag stack)"));
        }
        None => lines.push("[G] Deposit first bag stack (none)".into()),
    }
    match snap.bank.first() {
        Some(stack) => lines.push(format!(
            "[H] Withdraw {} (first bank slot)",
            instance_label(
                &stack.item_id,
                stack.count,
                stack.durability,
                stack.enchant_id.as_deref()
            )
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
        for (i, mail) in snap.mail.iter().enumerate() {
            let mut attachments = Vec::new();
            if mail.copper > 0 {
                attachments.push(format!("{}c", mail.copper));
            }
            if let Some(item_id) = &mail.item_id {
                attachments.push(instance_label(
                    item_id,
                    mail.item_count,
                    mail.durability,
                    mail.enchant_id.as_deref(),
                ));
            }
            let suffix = if attachments.is_empty() {
                String::new()
            } else {
                format!(" ({})", attachments.join(" + "))
            };
            lines.push(format!(
                "  [{}] #{} {} — {}{}",
                i + 1,
                mail.id,
                mail.from,
                mail.subject,
                suffix
            ));
        }
    }
    lines.push(format!(
        "[S] Send item · [Y] Send wallet copper · [P] Collect first mail · [1–9] Collect numbered mail · [X] Return first mail · Enter compose · postage {}c",
        snap.mail_postage
    ));
    lines.join("\n")
}

fn market_panel_text(snap: &TickSnapshot, ui: &UiFlags) -> String {
    let filtered = filtered_market(snap, &ui.market_filter);
    let pages = filtered.len().div_ceil(MARKET_PAGE_SIZE).max(1);
    let page = ui.market_page.min(pages.saturating_sub(1));
    let start = page * MARKET_PAGE_SIZE;
    let page_rows = filtered
        .get(start..filtered.len().min(start + MARKET_PAGE_SIZE))
        .unwrap_or(&[]);
    let search = if ui.market_searching {
        format!("Search> {}_", ui.market_filter)
    } else if ui.market_filter.is_empty() {
        "Filter: (all)   [/] search".into()
    } else {
        format!("Filter: {}   [/] search  [Esc] clear", ui.market_filter)
    };
    let mut lines = vec![
        "Market [U]".to_string(),
        format!(
            "Zone: {}   Copper: {}   Honor: {}",
            zone_name(snap),
            snap.progress.copper,
            snap.honor
        ),
        format!(
            "{search}   page {}/{}   duration {}h",
            page + 1,
            pages,
            ui.market_duration_hours
        ),
    ];
    if page_rows.is_empty() {
        lines.push("  (no listings)".into());
    } else {
        lines.extend(page_rows.iter().copied().map(listing_line));
    }
    match first_listable_bag_stack(snap) {
        Some((_, _, item_id, price)) => {
            let name = item(&item_id)
                .map(|d| d.name.to_string())
                .unwrap_or(item_id);
            let fee = match ui.market_duration_hours {
                24 => 10,
                48 => 20,
                _ => 5,
            };
            let start_bid = (price / 2).max(1);
            lines.push(format!(
                "[L] List 1×{name} bid {start_bid}c buyout {price}c (+{fee}c {}h fee)",
                ui.market_duration_hours
            ));
        }
        None => lines.push("[L] List first non-quest bag stack (none)".into()),
    }
    lines.push("[O] Buyout first affordable   [B] Bid first filtered".into());
    lines.push("[X] Cancel   [,][.] duration   [[][]] page".into());
    lines.join("\n")
}

fn guild_panel_text(snap: &TickSnapshot, compose: &str) -> String {
    let mut lines = vec!["Guild  [J] open · Esc close".into()];
    if let Some(inv) = &snap.guild_invite {
        lines.push(format!(
            "{} invited you to <{}>. Enter accept · Ctrl+X decline",
            inv.from_name, inv.guild_name
        ));
    }
    if let Some(g) = &snap.guild {
        lines.push(format!("<{}>  you: {}", g.name, g.rank));
        if !g.motd.is_empty() {
            lines.push(format!("MOTD ({}) {}", g.motd_set_by, g.motd));
        }
        for m in &g.members {
            let star = if m.online { "*" } else { " " };
            lines.push(format!("{star}{}  {}  {}", m.name, m.rank, m.level));
        }
        lines.push("Type to chat · /o officer · /motd text · Enter send".into());
        lines.push("/invite /kick /officer /member /transfer Name · Ctrl+Q leave".into());
        if g.rank == "leader" {
            lines.push("Ctrl+D disband · Ctrl+V/K/P/O/T if a player is targeted".into());
        } else if g.rank == "officer" {
            lines.push("/invite /kick Name · Ctrl+V/K if a player is targeted".into());
        }
    } else if snap.guild_invite.is_none() {
        lines.push("Type a name, Enter to found a guild (3-24 letters).".into());
    }
    lines.push(format!("> {compose}_"));
    lines.join("\n")
}

pub(crate) fn party_frames_text(snap: &TickSnapshot) -> String {
    let mut lines = Vec::new();
    if !snap.pending_invite_from.is_empty() {
        lines.push(format!(
            "{} invited you. O accept / P decline",
            snap.pending_invite_from
        ));
    }
    if let Some(rc) = &snap.ready_check {
        if !rc.you_responded {
            lines.push(format!(
                "Ready check {}/{}. O ready / P not ready",
                rc.ready_count, rc.total
            ));
        }
    }
    for m in &snap.party_members {
        if m.id == snap.player_id {
            continue;
        }
        let afk = if m.online { "" } else { " AFK" };
        let group = if snap.party_kind == "raid" {
            format!("G{} ", m.raid_group + 1)
        } else {
            String::new()
        };
        lines.push(format!(
            "{}{} {} {:.0}/{:.0}{}",
            group, m.class_id, m.name, m.hp, m.hp_max, afk
        ));
    }
    lines.join("\n")
}

pub(crate) fn party_panel_text(snap: &TickSnapshot) -> String {
    let mut lines = vec!["Party".into()];
    for m in &snap.party_members {
        let star = if Some(m.id) == snap.party_leader_id {
            "*"
        } else {
            " "
        };
        let afk = if m.online { "" } else { " AFK" };
        lines.push(format!("{star} {} {}{afk}", m.name, m.class_id));
    }
    lines.push("[X] Leave  [Y] Promote  [-] Kick  [R] Ready  [Backspace] Disband  [=] Raid".into());
    lines.join("\n")
}

fn listing_line(listing: &woc_protocol::MarketListingSnapshot) -> String {
    let mine = if listing.mine { " [yours]" } else { "" };
    let name = item(&listing.item_id)
        .map(|d| d.name.to_string())
        .unwrap_or_else(|| listing.item_id.clone());
    let mut extra = String::new();
    if let Some(def) = item(&listing.item_id) {
        if def.max_durability > 0 {
            let dur = listing.durability.unwrap_or(def.max_durability);
            extra.push_str(&format!(" {dur}/{}", def.max_durability));
        }
    }
    if let Some(eid) = listing.enchant_id.as_deref() {
        if let Some(edef) = enchant(eid) {
            extra.push_str(&format!(" [{}]", edef.name));
        }
    }
    let bid_bit = if listing.start_bid == 0 && listing.current_bid == 0 {
        String::new()
    } else if listing.current_bid > 0 {
        let who = listing.bidder.as_deref().unwrap_or("?");
        format!(" bid {}c ({who})", listing.current_bid)
    } else {
        format!(" start {}c", listing.start_bid)
    };
    format!(
        "  #{} {}×{name}{extra} — buyout {}c{} ({}){mine}",
        listing.id, listing.count, listing.price, bid_bit, listing.seller
    )
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
            ChromePanelKind::Guild => ui.show_guild,
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
            ChromePanelKind::Mail => {
                let recipient = if ui.mail_compose {
                    format!("To: {}_", ui.mail_to)
                } else {
                    format!("To: {}", ui.mail_to)
                };
                format!("{recipient}\n{}", mail_panel_text(&host.snapshot))
            }
            ChromePanelKind::Market => market_panel_text(&host.snapshot, &ui),
            ChromePanelKind::Guild => guild_panel_text(&host.snapshot, &ui.guild_compose),
        };
    }
}

pub(crate) fn format_quest_log_line(entry: &QuestLogEntry) -> String {
    let Some(def) = quest(&entry.quest_id) else {
        return format!("{} [{}]", entry.quest_id, entry.state);
    };
    let objs: Vec<String> = def
        .objectives
        .iter()
        .enumerate()
        .map(|(i, obj)| {
            let (label, need) = match obj {
                QuestObjective::Kill { label, count, .. } => (*label, *count),
                QuestObjective::Collect { label, count, .. } => (*label, *count),
                QuestObjective::Talk { label, .. }
                | QuestObjective::Explore { label, .. }
                | QuestObjective::Escort { label, .. } => (*label, 1u32),
            };
            let have = entry.counts.get(i).copied().unwrap_or(0);
            format!("{label} {have}/{need}")
        })
        .collect();
    format!("{} [{}] — {}", def.name, entry.state, objs.join("; "))
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
                **t = "Quests: (none — talk to a quest giver with E)".into();
            } else {
                let lines: Vec<String> = snap.quest_log.iter().map(format_quest_log_line).collect();
                **t = format!("Quests: {}  [X] abandon [Y] share", lines.join(" · "));
            }
        } else {
            let active = snap
                .quest_log
                .iter()
                .find(|q| q.state == "active" || q.state == "ready");
            let mut line = match active {
                Some(q) => format!("{} (L list)", format_quest_log_line(q)),
                None => "Quest: — (E talk · L list)".into(),
            };
            if let Some(ready) = snap.quest_log.iter().find(|q| q.state == "ready") {
                if let Some(def) = woc_content::quest(&ready.quest_id) {
                    if !def.reward.choices.is_empty() {
                        let picks: Vec<String> = def
                            .reward
                            .choices
                            .iter()
                            .enumerate()
                            .map(|(i, id)| format!("{} {id}", i + 1))
                            .collect();
                        line.push_str(&format!(" · choose {}", picks.join(" · ")));
                    }
                }
            }
            **t = line;
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
                        "  [{}] {} ({kind})",
                        s.slot + 1,
                        bag_stack_label(&s.item_id, s.count, s.durability)
                    ));
                }
                lines.push("[1-9] Equip/Use slot · [Q] first legal gear".into());
                lines.push("[F] Use first consumable".into());
                if snap.open_vendor.is_some()
                    || snap
                        .open_npc
                        .as_ref()
                        .is_some_and(|npc| npc.services.iter().any(|service| service == "vendor"))
                {
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
            "Character\nClass: {class}\nLevel: {}\nXP: {}/{}\nCopper: {}\nTalents: {} pts · {}\nEquipment:\n  Main: {}\n  Off: {}\n  Head: {}\n  Chest: {}\n  Legs: {}\n  Feet: {}\n  Neck: {}\n  Finger: {}\n  Finger2: {}\n  Shoulder: {}\n  Back: {}\n  Wrist: {}\n  Hands: {}\n  Waist: {}\n  Trinket: {}\n  Trinket2: {}\nAP: {:.0}   Armor: {:.0}   SP: {:.0}\n{}\n[1-9] Unequip  [0-=[]';] Extra slots",
            snap.progress.level,
            snap.progress.xp,
            snap.progress.xp_to_level,
            snap.progress.copper,
            snap.talent_points,
            talent_bonus_summary(snap),
            weapon_label(
                eq.main_hand.as_deref(),
                eq.main_hand_durability,
                eq.main_hand_quality.as_deref(),
                eq.main_hand_enchant.as_deref(),
            ),
            weapon_label(
                eq.off_hand.as_deref(),
                eq.off_hand_durability,
                eq.off_hand_quality.as_deref(),
                eq.off_hand_enchant.as_deref(),
            ),
            equipment_label(eq.head.as_deref(), eq.head_durability, eq.head_quality.as_deref()),
            equipment_label(eq.chest.as_deref(), eq.chest_durability, eq.chest_quality.as_deref()),
            equipment_label(eq.legs.as_deref(), eq.legs_durability, eq.legs_quality.as_deref()),
            equipment_label(eq.feet.as_deref(), eq.feet_durability, eq.feet_quality.as_deref()),
            equipment_label(eq.neck.as_deref(), None, eq.neck_quality.as_deref()),
            equipment_label(eq.finger.as_deref(), None, eq.finger_quality.as_deref()),
            equipment_label(eq.finger2.as_deref(), None, eq.finger2_quality.as_deref()),
            equipment_label(
                eq.shoulder.as_deref(),
                eq.shoulder_durability,
                eq.shoulder_quality.as_deref(),
            ),
            equipment_label(eq.back.as_deref(), eq.back_durability, eq.back_quality.as_deref()),
            equipment_label(eq.wrist.as_deref(), eq.wrist_durability, eq.wrist_quality.as_deref()),
            equipment_label(eq.hands.as_deref(), eq.hands_durability, eq.hands_quality.as_deref()),
            equipment_label(eq.waist.as_deref(), eq.waist_durability, eq.waist_quality.as_deref()),
            equipment_label(eq.trinket.as_deref(), None, eq.trinket_quality.as_deref()),
            equipment_label(eq.trinket2.as_deref(), None, eq.trinket2_quality.as_deref()),
            snap.attack_power,
            snap.armor,
            snap.spell_power,
            reputation_block(snap),
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

pub(crate) fn update_party_hud(
    host: Res<GameHost>,
    ui: Res<UiFlags>,
    mut frames: Query<&mut Text, With<HudPartyFrames>>,
    mut panel_text: Query<&mut Text, (With<HudPartyText>, Without<HudPartyFrames>)>,
    mut panel: Query<&mut Visibility, With<HudPartyPanel>>,
) {
    let snap = &host.snapshot;
    if let Ok(mut t) = frames.single_mut() {
        **t = party_frames_text(snap);
    }
    if let Ok(mut vis) = panel.single_mut() {
        *vis = if ui.show_party {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut t) = panel_text.single_mut() {
        **t = party_panel_text(snap);
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
        return format!(
            "[1] {name} {ready}   [2] —   [3] —   [4] —   [5] —{hint}",
            hint = class_interact_hint(snap)
        );
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

fn spawn_session_button(
    parent: &mut ChildSpawnerCommands<'_>,
    npc_id: EntityId,
    label: String,
    action: InteractAction,
    toast: String,
) {
    parent
        .spawn((
            Button,
            NpcSessionButton {
                npc_id,
                action,
                toast,
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

/// Show the NPC session panel when `open_npc` or legacy `open_vendor` is set.
pub(crate) fn sync_vendor_panel(
    mut commands: Commands,
    host: Res<GameHost>,
    mut cache: ResMut<VendorUiCache>,
    mut panel: Query<&mut Visibility, With<HudVendorPanel>>,
    mut title: Query<&mut Text, With<HudVendorTitle>>,
    offers_root: Query<Entity, With<HudVendorOffers>>,
) {
    let snap = &host.snapshot;
    match npc_session_cache_key(snap) {
        Some(key) => {
            if let Ok(mut vis) = panel.single_mut() {
                *vis = Visibility::Visible;
            }
            if let Ok(mut t) = title.single_mut() {
                **t = snap
                    .open_npc
                    .as_ref()
                    .map(|npc| npc.npc_name.clone())
                    .or_else(|| {
                        snap.open_vendor
                            .as_ref()
                            .map(|vendor| vendor.npc_name.clone())
                    })
                    .unwrap_or_else(|| "NPC".into());
            }
            if cache.key.as_ref() == Some(&key) {
                return;
            }
            cache.key = Some(key);
            let Ok(offers_e) = offers_root.single() else {
                return;
            };
            commands.entity(offers_e).despawn_related::<Children>();
            let npc_id = snap
                .open_npc
                .as_ref()
                .map(|npc| npc.npc_id)
                .or_else(|| snap.open_vendor.as_ref().map(|vendor| vendor.npc_id))
                .unwrap_or_default();
            let stock = npc_session_stock(snap);
            let stock_empty = stock.is_empty();
            let open_npc = snap.open_npc.clone();
            let help = npc_session_help(snap);
            commands.entity(offers_e).with_children(|parent| {
                if !help.is_empty() {
                    parent.spawn((
                        Text::new(help),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.78, 0.86, 0.72)),
                    ));
                }
                for offer in stock {
                    let label =
                        format!("Buy {} ×{} — {}c", offer.item_id, offer.count, offer.price);
                    let toast = format!("Buying {} ×{}", offer.item_id, offer.count.max(1));
                    spawn_session_button(
                        parent,
                        npc_id,
                        label,
                        InteractAction::Buy {
                            item_id: offer.item_id,
                            count: offer.count.max(1),
                        },
                        toast,
                    );
                }
                if let Some(npc) = open_npc {
                    if npc.can_repair {
                        spawn_session_button(
                            parent,
                            npc.npc_id,
                            format!("Repair — {}c", npc.repair_cost),
                            InteractAction::RepairAll,
                            "Repairing gear.".into(),
                        );
                    }
                    for profession in npc.train_professions {
                        spawn_session_button(
                            parent,
                            npc.npc_id,
                            format!("Train {profession}"),
                            InteractAction::TrainProfession {
                                id: profession.clone(),
                            },
                            format!("Training {profession}."),
                        );
                    }
                    if npc
                        .services
                        .iter()
                        .any(|service| service == "class_trainer")
                    {
                        spawn_session_button(
                            parent,
                            npc.npc_id,
                            "Train class".into(),
                            InteractAction::TrainClass,
                            "Training class.".into(),
                        );
                    }
                    if npc.can_bind {
                        spawn_session_button(
                            parent,
                            npc.npc_id,
                            "Bind hearth".into(),
                            InteractAction::BindHearth,
                            "Binding hearth.".into(),
                        );
                    }
                    for row in npc.buyback {
                        spawn_session_button(
                            parent,
                            npc.npc_id,
                            format!("Buyback {} ×{} — {}c", row.item_id, row.count, row.price),
                            InteractAction::Buyback { slot: row.slot },
                            format!("Buying back {} ×{}.", row.item_id, row.count),
                        );
                    }
                }
                if stock_empty && snap.open_npc.is_none() {
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

pub(crate) fn npc_session_clicks(
    interactions: Query<(&Interaction, &NpcSessionButton), Changed<Interaction>>,
    mut host: ResMut<GameHost>,
) {
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        host.interact(btn.npc_id, btn.action.clone());
        host.recent_toasts.push((btn.toast.clone(), 1.5));
    }
}

/// Release look-grab while an NPC session is open so panel buttons are clickable.
pub(crate) fn vendor_ungrab_cursor(
    mut host: ResMut<GameHost>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if (host.snapshot.open_vendor.is_none() && host.snapshot.open_npc.is_none())
        || !host.cursor_grabbed
    {
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
        InvSlotSnapshot, MailSnapshot, MarketListingSnapshot, NpcSessionSnapshot,
        ProfessionSkillSnapshot, TalentRankSnapshot, TickSnapshot,
    };

    fn chrome_snapshot() -> TickSnapshot {
        let mut snap = TickSnapshot::default();
        snap.progress.class_id = "warrior".into();
        snap.progress.copper = 75;
        snap.zone_id = "eastbrook".into();
        snap.honor = 12;
        snap.talent_points = 2;
        snap.mail_postage = 1;
        snap.talents.push(TalentRankSnapshot {
            talent_id: "warrior_cruelty".into(),
            rank: 1,
        });
        snap.professions.push(ProfessionSkillSnapshot {
            id: "herbalism".into(),
            skill: 18,
        });
        // Quest item first: bank G must skip it and pick the non-quest stack below.
        snap.inventory.push(InvSlotSnapshot {
            slot: 0,
            item_id: "boar_tusk".into(),
            count: 1,
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
        snap.bank.push(InvSlotSnapshot {
            slot: 0,
            item_id: "silverleaf".into(),
            count: 4,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
        });
        // Worn bank stack: proves bank lines carry durability through storage.
        snap.bank.push(InvSlotSnapshot {
            slot: 1,
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(12),
            enchant_id: None,
            quality: None,
            bound: false,
        });
        snap.mail.push(MailSnapshot {
            id: 7,
            from: "Ada".into(),
            subject: "Parcel".into(),
            copper: 9,
            item_id: Some("baked_bread".into()),
            item_count: 2,
            ..Default::default()
        });
        snap.market.push(MarketListingSnapshot {
            id: 11,
            seller: "Grace".into(),
            item_id: "peacebloom".into(),
            count: 5,
            price: 30,
            mine: false,
            durability: None,
            enchant_id: None,
            quality: None,
            expires_tick: 0,
            start_bid: 0,
            current_bid: 0,
            bidder: None,
            bound: false,
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
    fn bank_panel_offers_first_non_quest_deposit() {
        let text = bank_panel_text(&chrome_snapshot());
        assert!(text.contains("[G] Deposit"));
        assert!(!text.contains("first bag junk"));
    }

    #[test]
    fn bank_panel_shows_durability_on_worn_stored_gear() {
        let text = bank_panel_text(&chrome_snapshot());
        assert!(text.contains("Worn Sword 12/"));
    }

    #[test]
    fn mail_panel_formats_attachments_and_collect_help() {
        let text = mail_panel_text(&chrome_snapshot());

        assert!(text.contains("#7 Ada — Parcel"));
        assert!(text.contains("9c + 2×baked_bread"));
        assert!(text.contains("[P] Collect first mail"));
    }

    #[test]
    fn mail_panel_shows_send_and_numbered_collect() {
        let text = mail_panel_text(&chrome_snapshot());
        assert!(text.contains("[S] Send item"));
        assert!(text.contains("[P] Collect first mail"));
        assert!(text.contains("[1–9] Collect numbered"));
        assert!(text.contains("[X] Return"));
    }

    #[test]
    fn mail_panel_shows_postage_from_snapshot() {
        let text = mail_panel_text(&chrome_snapshot());
        assert!(text.contains("postage 1c"));
    }

    #[test]
    fn market_panel_formats_listings_wallet_and_buy_help() {
        let text = market_panel_text(&chrome_snapshot(), &UiFlags::default());

        assert!(text.contains("Copper: 75"));
        assert!(text.contains("#11 5×Peacebloom — buyout 30c (Grace)"));
        assert!(text.contains("[O] Buyout first affordable"));
        assert!(text.contains("[L] List"));
        assert!(text.contains("[X] Cancel"));
        assert!(text.contains("[B] Bid"));
        assert!(text.contains("duration 12h"));
    }

    #[test]
    fn first_listable_bag_stack_skips_quest_and_allows_weapons() {
        let mut snap = TickSnapshot::default();
        snap.inventory.push(InvSlotSnapshot {
            slot: 0,
            item_id: "boar_tusk".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
        });
        snap.inventory.push(InvSlotSnapshot {
            slot: 1,
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
            quality: None,
            bound: false,
        });
        let listed = first_listable_bag_stack(&snap).unwrap();
        assert_eq!(listed.0, 1);
        assert_eq!(listed.2, "worn_sword");
    }

    #[test]
    fn first_listable_bag_stack_skips_bound() {
        let mut snap = TickSnapshot::default();
        snap.inventory.push(InvSlotSnapshot {
            slot: 0,
            item_id: "silverleaf".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: true,
        });
        snap.inventory.push(InvSlotSnapshot {
            slot: 1,
            item_id: "wolf_fang".into(),
            count: 1,
            durability: None,
            enchant_id: None,
            quality: None,
            bound: false,
        });
        let listed = first_listable_bag_stack(&snap).unwrap();
        assert_eq!(listed.2, "wolf_fang");
    }

    #[test]
    fn filtered_market_pages_by_name() {
        let mut snap = chrome_snapshot();
        snap.market.push(MarketListingSnapshot {
            id: 12,
            seller: "Ada".into(),
            item_id: "silverleaf".into(),
            count: 1,
            price: 8,
            mine: false,
            durability: None,
            enchant_id: None,
            quality: None,
            expires_tick: 0,
            start_bid: 4,
            current_bid: 0,
            bidder: None,
            bound: false,
        });
        let hits = filtered_market(&snap, "peace");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item_id, "peacebloom");
        assert_eq!(listing_min_bid(&snap.market[1]), Some(4));
        assert_eq!(cycle_duration_hours(12, true), 24);
        assert_eq!(cycle_duration_hours(48, true), 12);
    }

    #[test]
    fn market_panel_shows_wear_and_enchant() {
        let mut snap = chrome_snapshot();
        snap.market[0].item_id = "worn_sword".into();
        snap.market[0].count = 1;
        snap.market[0].durability = Some(7);
        snap.market[0].enchant_id = Some("coarse_sharpening".into());
        let text = market_panel_text(&snap, &UiFlags::default());
        assert!(text.contains("Worn Sword"));
        assert!(text.contains("7/40"));
        assert!(text.contains("Coarse Sharpening"));
    }

    #[test]
    fn npc_session_help_mentions_auction_when_can_auction() {
        let mut snap = TickSnapshot::default();
        snap.open_npc = Some(NpcSessionSnapshot {
            npc_id: 9,
            npc_name: "Auctioneer Lise".into(),
            greeting: String::new(),
            services: vec!["auctioneer".into()],
            stock: vec![],
            train_professions: vec![],
            can_repair: false,
            repair_cost: 0,
            can_bind: false,
            buyback: vec![],
            can_auction: true,
            can_bank: false,
            can_mail: false,
            discount_pct: 0,
        });
        let text = npc_session_help(&snap);
        assert!(text.contains("[U] Auction"));
    }

    #[test]
    fn npc_session_help_mentions_repair_when_session_can_repair() {
        let mut snap = TickSnapshot::default();
        snap.open_npc = Some(NpcSessionSnapshot {
            npc_id: 42,
            npc_name: "Smith Brann".into(),
            greeting: String::new(),
            services: vec![],
            stock: vec![],
            train_professions: vec![],
            can_repair: true,
            repair_cost: 12,
            can_bind: false,
            buyback: vec![],
            can_auction: false,
            can_bank: false,
            can_mail: false,
            discount_pct: 0,
        });

        let text = npc_session_help(&snap);

        assert!(text.contains("[R] Repair"));
    }

    #[test]
    fn reputation_block_lists_standing() {
        let mut snap = TickSnapshot::default();
        snap.reputation.push(woc_protocol::ReputationSnapshot {
            faction_id: "eastbrook_watch".into(),
            name: "Eastbrook Watch".into(),
            value: 500,
            standing: "friendly".into(),
        });
        let text = reputation_block(&snap);
        assert!(text.contains("Eastbrook Watch"));
        assert!(text.contains("Friendly"));
        assert!(text.contains("500"));
    }

    #[test]
    fn quest_log_line_uses_name_and_counts() {
        use super::format_quest_log_line;
        use woc_protocol::QuestLogEntry;

        let line = format_quest_log_line(&QuestLogEntry {
            quest_id: "wolves_at_the_gate".into(),
            state: "active".into(),
            counts: vec![1],
        });
        assert!(line.contains("Wolves at the Gate"));
        assert!(line.contains("active"));
        assert!(line.contains("1/3"));
        assert!(line.contains("Young Wolves slain"));
        assert!(!line.starts_with("wolves_at_the_gate"));
    }

    #[test]
    fn quest_log_line_covers_explore() {
        use super::format_quest_log_line;
        use woc_protocol::QuestLogEntry;

        let line = format_quest_log_line(&QuestLogEntry {
            quest_id: "scout_north_road".into(),
            state: "active".into(),
            counts: vec![0],
        });
        assert!(line.contains("Scout the North Road"));
        assert!(line.contains("North road scouted 0/1"));
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
        snap.ability_bar = vec![woc_protocol::AbilityBarSlot {
            slot: 1,
            ability_id: "heroic_strike".into(),
            name: "Heroic Strike".into(),
            known: true,
            ready: true,
            cooldown: 0.0,
        }];
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

    #[test]
    fn party_frames_format_other_members() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.party_kind = "party".into();
        snap.party_leader_id = Some(1);
        snap.party_members = vec![
            woc_protocol::PartyMemberSnapshot {
                id: 1,
                name: "Alice".into(),
                class_id: "warrior".into(),
                hp: 100.0,
                hp_max: 100.0,
                online: true,
                raid_group: 0,
            },
            woc_protocol::PartyMemberSnapshot {
                id: 2,
                name: "Bob".into(),
                class_id: "mage".into(),
                hp: 40.0,
                hp_max: 80.0,
                online: false,
                raid_group: 0,
            },
        ];
        let text = party_frames_text(&snap);
        assert!(text.contains("Bob"));
        assert!(text.contains("40/80"));
        assert!(text.contains("AFK"));
        assert!(!text.contains("Alice"));
        let panel = party_panel_text(&snap);
        assert!(panel.contains("*"));
        assert!(panel.contains("[X] Leave"));
    }

    #[test]
    fn raid_frames_group_two_on_second_column() {
        let mut snap = TickSnapshot::default();
        snap.player_id = 1;
        snap.party_kind = "raid".into();
        snap.party_members = (1..=6)
            .map(|id| woc_protocol::PartyMemberSnapshot {
                id,
                name: format!("P{id}"),
                class_id: "warrior".into(),
                hp: 10.0,
                hp_max: 10.0,
                online: true,
                raid_group: if id <= 5 { 0 } else { 1 },
            })
            .collect();
        let text = party_frames_text(&snap);
        assert!(text.contains("G2"));
        assert!(text.contains("P6"));
    }

    #[test]
    fn jewelry_equipment_label_uses_quality_and_name() {
        assert_eq!(
            equipment_label(Some("fang_pendant"), None, None),
            "Uncommon Fang Pendant"
        );
        assert_eq!(
            weapon_label(Some("worn_sword"), None, None, Some("coarse_sharpening"),),
            "Worn Sword 40/40 [Coarse Sharpening]"
        );
        assert_eq!(
            equipment_label(Some("wool_cloak"), None, None),
            "Uncommon Wool Cloak 30/30"
        );
    }
}
