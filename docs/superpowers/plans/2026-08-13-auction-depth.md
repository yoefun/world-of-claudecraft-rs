# Auction Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.14.0` / `auction-depth`: Eastbrook Auctioneer Lise, slot-accurate listings that keep durability and enchant, 5% house cut, and mail-always sale proceeds.

**Architecture:** `AuctionHouse` stays a `Sim` realm resource. Listings copy `InvStack` fields. `WorldHost::interact` gates `MarketList` / `MarketBuy` / `MarketCancel` on an in-range auctioneer session; `AuctionHouse` methods stay ungated for unit tests. Mail attachments carry the same instance fields. Protocol rev stays 8 with serde defaults.

**Tech Stack:** Rust 2021 workspace crates (`woc-content`, `woc-protocol`, `woc-sim`, `woc-persist`, `woc-server`, `woc-client`). No new dependencies. No Bevy inside sim/content.

**Design spec:** `docs/superpowers/specs/2026-08-13-auction-depth-design.md`

## Global Constraints

- `woc-sim` and `woc-content` MUST NOT depend on Bevy, `bevy_ecs`, wgpu, axum, or tokio.
- Client never decides listing success, house cut, or mail contents.
- Listing TTL is `LISTING_TTL_TICKS = 72_000` (`Sim.tick`), never wall clock.
- Tick fingerprint must remain `3214741777866168171u64`. No new named tick phase.
- `PROTOCOL_REV` stays `8`. New snapshot / DTO fields use `#[serde(default)]`.
- Upstream pin stays `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- English-only player-facing strings (exact copies from the spec).
- Auction listings stay on `Sim.market`. Do not reintroduce a fat `Entity` or add a listings actor column.
- Direct `AuctionHouse::{list_item,buy,cancel}` stay ungated. Only `WorldHost::interact` requires Auctioneer Lise.
- `grant_into(item_id, count)` keeps “fresh stack” behavior (`InvStack::new`). Market buy / mail collect of attachments use `grant_stack`.
- Every task ends with `cargo test --workspace --exclude woc-client` green, and `cargo check -p woc-client` green when client files change.
- Do not bump workspace `version` / `VERSION.toml` until Task 9 (implementation wave tag `1.14.0`).
- Author for commits: keep the repo’s existing style.

---

## File map (create / own)

| Path | Responsibility |
| --- | --- |
| `crates/woc-content/src/npcs.rs` | `NpcService::Auctioneer`, `is_auctioneer()`, `auctioneer_lise` |
| `crates/woc-content/src/zone1.rs` | Eastbrook spot `(4.0, 6.0)` |
| `crates/woc-content/src/lib.rs` | Roster + integrity tests |
| `crates/woc-sim/src/inventory.rs` | `take_from_slot`, `grant_stack`; `grant_into` wraps `grant_stack(InvStack::new)` |
| `crates/woc-protocol/src/lib.rs` | Additive listing / mail / `can_auction` fields |
| `crates/woc-sim/src/market.rs` | Instance listing, quest block, cut, mail-always proceeds |
| `crates/woc-sim/src/mail.rs` | Attachment `InvStack`; `grant_stack` on collect |
| `crates/woc-sim/src/interaction.rs` | `opens_npc_session`, `service_name`, `can_auction` |
| `crates/woc-sim/src/host.rs` | Auctioneer session gate |
| `crates/woc-persist/src/economy.rs` | DTO fields |
| `crates/woc-server/src/bridge.rs` | Copy instance fields both ways |
| `crates/woc-client/src/{hud,input,nameplates,visuals,map}.rs` | `[A]`, Talk opens U, list any non-quest, wear/enchant lines |
| `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md`, `CHANGELOG.md`, `VERSION.toml`, `Cargo.toml` | 1.14.0 tag (Task 9) |

---

### Task 1: Auctioneer content + Eastbrook spot

**Files:**
- Modify: `crates/woc-content/src/npcs.rs`
- Modify: `crates/woc-content/src/zone1.rs`
- Modify: `crates/woc-content/src/lib.rs`
- Modify: `crates/woc-sim/src/interaction.rs` (`opens_npc_session`, `service_name`)

**Interfaces:**
- Consumes: existing `NpcService`, `EASTBROOK.npcs`, `opens_npc_session`
- Produces: `NpcService::Auctioneer`, `NpcDef::is_auctioneer(&self) -> bool`, NPC id `auctioneer_lise`, `service_name(Auctioneer) == "auctioneer"`

- [ ] **Step 1: Write the failing content tests**

In `crates/woc-content/src/lib.rs`, extend `npc_services_roster_locked` and add:

```rust
#[test]
fn auctioneer_lise_is_eastbrook_auction_only() {
    let lise = npc("auctioneer_lise").expect("auctioneer_lise");
    assert!(lise.is_auctioneer());
    assert!(!lise.is_vendor());
    assert!(!lise.can_repair());
    assert!(lise.vendor_stock.is_empty());
    assert!(lise.trains.is_empty());
    assert!(EASTBROOK.npcs.iter().any(|s| s.npc_id == "auctioneer_lise"
        && (s.x - 4.0).abs() < f32::EPSILON
        && (s.z - 6.0).abs() < f32::EPSILON));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-content auctioneer_lise_is_eastbrook_auction_only --offline`

Expected: FAIL compiling (`is_auctioneer` / `auctioneer_lise` not found).

- [ ] **Step 3: Implement content**

In `npcs.rs` add the enum variant and helper:

```rust
pub enum NpcService {
    QuestGiver,
    Vendor,
    Repair,
    ProfessionTrainer,
    ClassTrainer,
    Innkeeper,
    Auctioneer,
}

impl NpcDef {
    pub fn is_auctioneer(&self) -> bool {
        self.services.contains(&NpcService::Auctioneer)
    }
}
```

Append to `ZONE1_NPCS`:

```rust
NpcDef {
    id: "auctioneer_lise",
    name: "Auctioneer Lise",
    greeting: "List it. The house takes its cut.",
    services: &[NpcService::Auctioneer],
    vendor_stock: &[],
    trains: &[],
},
```

In `zone1.rs` `EASTBROOK.npcs` append:

```rust
NpcSpot {
    npc_id: "auctioneer_lise",
    x: 4.0,
    z: 6.0,
},
```

In `interaction.rs`:

```rust
fn opens_npc_session(def: &NpcDef) -> bool {
    def.is_vendor()
        || def.can_repair()
        || def.is_profession_trainer()
        || def.is_class_trainer()
        || def.is_innkeeper()
        || def.is_auctioneer()
}

fn service_name(service: NpcService) -> &'static str {
    match service {
        NpcService::Vendor => "vendor",
        NpcService::Repair => "repair",
        NpcService::ProfessionTrainer => "profession_trainer",
        NpcService::ClassTrainer => "class_trainer",
        NpcService::Innkeeper => "innkeeper",
        NpcService::QuestGiver => "quest_giver",
        NpcService::Auctioneer => "auctioneer",
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-content --offline`

Expected: PASS, including `npc_services_roster_locked` and `auctioneer_lise_is_eastbrook_auction_only`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-content/src/npcs.rs crates/woc-content/src/zone1.rs crates/woc-content/src/lib.rs crates/woc-sim/src/interaction.rs
git commit -m "feat(content): add Eastbrook Auctioneer Lise"
```

---

### Task 2: `take_from_slot` and `grant_stack`

**Files:**
- Modify: `crates/woc-sim/src/inventory.rs`

**Interfaces:**
- Consumes: `InvStack { item_id, count, durability, enchant_id }`, existing stack-size / unstacked weapon-armor rule
- Produces: `take_from_slot(inv, slot, count) -> Option<InvStack>`, `grant_stack(inv, incoming) -> bool`; `grant_into` becomes `grant_stack(inv, InvStack::new(item_id, count))`

- [ ] **Step 1: Write the failing tests**

At the bottom of `inventory.rs` tests (add a `#[cfg(test)]` module if missing; otherwise extend it):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::InvStack;

    fn empty_bags() -> [Option<InvStack>; 4] {
        [None, None, None, None]
    }

    #[test]
    fn take_from_slot_preserves_wear_and_leaves_the_other_stack() {
        let mut inv = empty_bags();
        inv[0] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 3,
            durability: None,
            enchant_id: None,
        });
        inv[1] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 2,
            durability: None,
            enchant_id: None,
        });
        let taken = take_from_slot(&mut inv, 1, 1).unwrap();
        assert_eq!(taken.count, 1);
        assert_eq!(inv[0].as_ref().unwrap().count, 3);
        assert_eq!(inv[1].as_ref().unwrap().count, 1);
    }

    #[test]
    fn grant_stack_keeps_worn_enchanted_sword_unmerged() {
        let mut inv = empty_bags();
        let worn = InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
        };
        assert!(grant_stack(&mut inv, worn.clone()));
        assert_eq!(inv[0], Some(worn));
        assert!(grant_into(&mut inv, "worn_sword", 1));
        assert_eq!(inv[1].as_ref().unwrap().durability, Some(40));
        assert!(inv[1].as_ref().unwrap().enchant_id.is_none());
    }
}
```

If `inventory.rs` has no test module yet, this creates it. `worn_sword` `max_durability` is 40.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim take_from_slot_preserves_wear --offline`

Expected: FAIL compiling (`take_from_slot` / `grant_stack` not found).

- [ ] **Step 3: Implement helpers**

```rust
pub fn take_from_slot(inv: &mut [Option<InvStack>], slot: u8, count: u32) -> Option<InvStack> {
    let stack = inv.get_mut(slot as usize)?.as_mut()?;
    let take = count.min(stack.count).max(1);
    if take > stack.count {
        return None;
    }
    let taken = InvStack {
        item_id: stack.item_id.clone(),
        count: take,
        durability: stack.durability,
        enchant_id: stack.enchant_id.clone(),
    };
    stack.count -= take;
    if stack.count == 0 {
        inv[slot as usize] = None;
    }
    Some(taken)
}

pub fn grant_stack(inv: &mut [Option<InvStack>], incoming: InvStack) -> bool {
    if incoming.count == 0 {
        return true;
    }
    let stack_size = woc_content::item(&incoming.item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(&incoming.item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    let max_stack = if unstacked { 1 } else { stack_size };
    let mut remaining = incoming.count;
    if max_stack > 1 {
        for stack in inv.iter_mut().flatten() {
            if stack.item_id == incoming.item_id
                && stack.durability == incoming.durability
                && stack.enchant_id == incoming.enchant_id
                && stack.count < max_stack
            {
                let space = max_stack - stack.count;
                let add = remaining.min(space);
                stack.count += add;
                remaining -= add;
                if remaining == 0 {
                    return true;
                }
            }
        }
    }
    while remaining > 0 {
        let Some(empty) = inv.iter_mut().find(|s| s.is_none()) else {
            return false;
        };
        let add = remaining.min(max_stack);
        *empty = Some(InvStack {
            item_id: incoming.item_id.clone(),
            count: add,
            durability: incoming.durability,
            enchant_id: incoming.enchant_id.clone(),
        });
        remaining -= add;
    }
    true
}

pub fn grant_into(inv: &mut [Option<InvStack>], item_id: &str, count: u32) -> bool {
    grant_stack(inv, InvStack::new(item_id, count))
}
```

Delete the old `grant_into` body (the merge loop). Keep `remove_item` unchanged.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --offline`

Expected: PASS, including existing loot/vendor tests that call `grant_into`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/inventory.rs
git commit -m "feat(sim): preserve item instance when moving bag stacks"
```

---

### Task 3: Additive protocol fields

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`
- Modify: `crates/woc-client/src/hud.rs` (struct literals only, so the crate compiles)

**Interfaces:**
- Consumes: existing `MarketListingSnapshot`, `MailSnapshot`, `NpcSessionSnapshot`
- Produces: `durability: Option<u32>`, `enchant_id: Option<String>` on market + mail snapshots; `expires_tick: u64` on market; `can_auction: bool` on NPC session. All `#[serde(default)]`. `PROTOCOL_REV` remains 8.

- [ ] **Step 1: Write the failing omit-key test**

In `crates/woc-protocol/src/lib.rs` tests, next to `finger2_and_enchant_defaults`:

```rust
#[test]
fn market_mail_auction_fields_default_when_omitted() {
    let listing: MarketListingSnapshot =
        serde_json::from_str(r#"{"id":1,"seller":"Ada","item_id":"x","count":1,"price":2}"#)
            .unwrap();
    assert!(listing.durability.is_none());
    assert!(listing.enchant_id.is_none());
    assert_eq!(listing.expires_tick, 0);
    assert!(!listing.mine);

    let mail: MailSnapshot =
        serde_json::from_str(r#"{"id":1,"from":"AH","subject":"Sold","copper":40,"item_id":null,"item_count":0}"#)
            .unwrap();
    assert!(mail.durability.is_none());
    assert!(mail.enchant_id.is_none());

    let session: NpcSessionSnapshot = serde_json::from_str(
        r#"{"npc_id":1,"npc_name":"Lise"}"#,
    )
    .unwrap();
    assert!(!session.can_auction);
    assert_eq!(PROTOCOL_REV, 8);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-protocol market_mail_auction_fields_default_when_omitted --offline`

Expected: FAIL compiling (unknown fields / missing fields in struct).

- [ ] **Step 3: Add fields**

```rust
pub struct NpcSessionSnapshot {
    // existing fields…
    #[serde(default)]
    pub can_auction: bool,
}

pub struct MailSnapshot {
    // existing fields…
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
}

pub struct MarketListingSnapshot {
    // existing fields…
    #[serde(default)]
    pub durability: Option<u32>,
    #[serde(default)]
    pub enchant_id: Option<String>,
    #[serde(default)]
    pub expires_tick: u64,
}
```

Update every struct literal so the workspace compiles:

- `crates/woc-sim/src/interaction.rs` `npc_session_snapshot` → `can_auction: def.is_auctioneer()`
- `crates/woc-sim/src/market.rs` both snapshot constructors → `durability: None`, `enchant_id: None`, `expires_tick: l.expires_tick` (Task 4 overwrites durability/enchant from the listing)
- `crates/woc-sim/src/mail.rs` `snapshot_for_entity` → `durability: None`, `enchant_id: None` (Task 6 copies from `MailItem`)
- `crates/woc-client/src/hud.rs` `chrome_snapshot` + `npc_session_help` test literal (`can_auction: false`, listing/mail instance fields `None` / `0`)
- Any full snapshot literal in `woc-protocol` tests that constructs these structs by field

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-protocol --offline && cargo test -p woc-sim --offline && cargo test -p woc-client --offline`

Expected: PASS (`woc-client` unit tests do not need GPU).

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs crates/woc-sim/src/interaction.rs crates/woc-sim/src/market.rs crates/woc-sim/src/mail.rs crates/woc-client/src/hud.rs
git commit -m "feat(protocol): additive auction instance fields on rev 8"
```

---

### Task 4: List by slot, quest block, instance payload

**Files:**
- Modify: `crates/woc-sim/src/market.rs`

**Interfaces:**
- Consumes: `take_from_slot`, `Listing` plus `durability` / `enchant_id`
- Produces: `list_item` takes the named slot; quest items toast `"This item is needed for a quest."`; listing stores instance fields

- [ ] **Step 1: Write the failing tests**

In `market.rs` tests:

```rust
#[test]
fn list_takes_the_named_slot_not_another_stack_of_the_same_id() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 100;
    }
    if let Some(bags) = world.get_mut::<Bags>(1) {
        bags.inventory[0] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 3,
            durability: None,
            enchant_id: None,
        });
        bags.inventory[1] = Some(InvStack {
            item_id: "silverleaf".into(),
            count: 2,
            durability: None,
            enchant_id: None,
        });
    }
    let mut ah = AuctionHouse::new();
    let mut events = Vec::new();
    assert!(ah.list_item(&mut world, 1, 1, 1, 12, 1, &mut events));
    let bags = world.get::<Bags>(1).unwrap();
    assert_eq!(bags.inventory[0].as_ref().unwrap().count, 3);
    assert_eq!(bags.inventory[1].as_ref().unwrap().count, 1);
    assert_eq!(ah.listings[0].count, 1);
}

#[test]
fn list_refuses_quest_items() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 100;
    }
    if let Some(bags) = world.get_mut::<Bags>(1) {
        assert!(grant_into(&mut bags.inventory, "boar_tusk", 1));
    }
    let slot = world
        .get::<Bags>(1)
        .unwrap()
        .inventory
        .iter()
        .position(|s| s.as_ref().is_some_and(|st| st.item_id == "boar_tusk"))
        .unwrap() as u8;
    let mut ah = AuctionHouse::new();
    let mut events = Vec::new();
    assert!(!ah.list_item(&mut world, 1, slot, 1, 20, 1, &mut events));
    assert!(ah.listings.is_empty());
    assert_eq!(world.get::<Progress>(1).unwrap().copper, 100);
    assert!(events.iter().any(|e| matches!(
        e,
        SimEvent::Toast { message } if message == "This item is needed for a quest."
    )));
}

#[test]
fn list_stores_durability_and_enchant() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 100;
    }
    if let Some(bags) = world.get_mut::<Bags>(1) {
        bags.inventory[0] = Some(InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
        });
    }
    let mut ah = AuctionHouse::new();
    let mut events = Vec::new();
    assert!(ah.list_item(&mut world, 1, 0, 1, 25, 1, &mut events));
    assert_eq!(ah.listings[0].durability, Some(7));
    assert_eq!(
        ah.listings[0].enchant_id.as_deref(),
        Some("coarse_sharpening")
    );
}
```

Add `InvStack` and `grant_into` imports in the test module. Add `durability` / `enchant_id` to every `Listing { ... }` literal in existing tests (`None`, `None`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim list_refuses_quest_items --offline`

Expected: FAIL (quest item currently lists, or `Listing` lacks fields).

- [ ] **Step 3: Implement list_item changes**

On `Listing`:

```rust
pub durability: Option<u32>,
pub enchant_id: Option<String>,
```

Replace the `remove_item` path in `list_item` with:

```rust
let kind = woc_content::item(&stack.item_id).map(|d| d.kind);
if kind == Some(woc_content::ItemKind::Quest) {
    events.push(SimEvent::Toast {
        message: "This item is needed for a quest.".into(),
    });
    return false;
}
let take = count.min(stack.count).max(1);
let taken = {
    let Some(bags) = world.get_mut::<Bags>(seller) else {
        return false;
    };
    match crate::inventory::take_from_slot(&mut bags.inventory, bag_slot, take) {
        Some(s) => s,
        None => return false,
    }
};
```

Push listing with `item_id: taken.item_id`, `count: taken.count`, `durability: taken.durability`, `enchant_id: taken.enchant_id`.

In `snapshot_public` / `snapshot_for`, set `durability: l.durability.clone()` (it's `Option<u32>`, copy), `enchant_id: l.enchant_id.clone()`, `expires_tick: l.expires_tick`.

Keep reading the slot first for the empty-slot toast **before** the quest check (so an empty slot still says `"Empty bag slot."`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --offline`

Expected: PASS, including `list_buy_and_cancel_flow`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/market.rs
git commit -m "feat(sim): list the named bag stack with wear and enchant"
```

---

### Task 5: House cut and mail-always proceeds

**Files:**
- Modify: `crates/woc-sim/src/market.rs`
- Modify: `crates/woc-sim/src/mail.rs` (only if `deliver_system` signature changes in Task 6; if Task 6 is not done yet, keep current copper-only `deliver_system` and pass `None, 0` for the item)

**Interfaces:**
- Consumes: `sale_cut(price) = price / 20`, `sale_proceeds(price) = price - cut`
- Produces: buyer pays full price; seller never gains `Progress.copper` on sale; inbox copper equals proceeds

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn sale_cut_is_five_percent_floored() {
    assert_eq!(sale_cut(50), 2);
    assert_eq!(sale_proceeds(50), 48);
    assert_eq!(sale_cut(19), 0);
    assert_eq!(sale_proceeds(19), 19);
}

#[test]
fn buy_always_mails_proceeds_even_when_seller_is_online() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 100;
    }
    if let Some(p) = world.get_mut::<Progress>(2) {
        p.copper = 200;
    }
    let mut ah = AuctionHouse::new();
    ah.listings.push(Listing {
        id: 1,
        seller_id: 1,
        seller_durable: "ada".into(),
        seller_name: "Ada".into(),
        item_id: "silverleaf".into(),
        count: 1,
        durability: None,
        enchant_id: None,
        price: 50,
        expires_tick: 9999,
    });
    ah.set_next_id(2);
    let mut mail = Mailbox::new();
    let mut events = Vec::new();
    assert!(ah.buy(&mut world, &mut mail, 2, 1, &mut events));
    assert_eq!(world.get::<Progress>(1).unwrap().copper, 100);
    assert_eq!(world.get::<Progress>(2).unwrap().copper, 150);
    assert_eq!(mail.all_mails().len(), 1);
    assert_eq!(mail.all_mails()[0].copper, 48);
    assert_eq!(mail.all_mails()[0].subject, "Auction sold");
}

#[test]
fn buy_grants_the_listed_wear_and_enchant() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
    if let Some(p) = world.get_mut::<Progress>(2) {
        p.copper = 200;
    }
    let mut ah = AuctionHouse::new();
    ah.listings.push(Listing {
        id: 1,
        seller_id: 1,
        seller_durable: "ada".into(),
        seller_name: "Ada".into(),
        item_id: "worn_sword".into(),
        count: 1,
        durability: Some(7),
        enchant_id: Some("coarse_sharpening".into()),
        price: 40,
        expires_tick: 9999,
    });
    ah.set_next_id(2);
    let mut mail = Mailbox::new();
    let mut events = Vec::new();
    assert!(ah.buy(&mut world, &mut mail, 2, 1, &mut events));
    let sword = world
        .get::<Bags>(2)
        .unwrap()
        .inventory
        .iter()
        .flatten()
        .find(|s| s.item_id == "worn_sword")
        .unwrap();
    assert_eq!(sword.durability, Some(7));
    assert_eq!(sword.enchant_id.as_deref(), Some("coarse_sharpening"));
}
```

Export:

```rust
pub const SALE_CUT_NUM: u32 = 1;
pub const SALE_CUT_DEN: u32 = 20;

pub fn sale_cut(price: u32) -> u32 {
    price / SALE_CUT_DEN
}

pub fn sale_proceeds(price: u32) -> u32 {
    price.saturating_sub(sale_cut(price))
}
```

(`SALE_CUT_NUM` is documentation; cut is `price / 20`. Do not multiply by NUM.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim buy_always_mails_proceeds_even_when_seller_is_online --offline`

Expected: FAIL (`Progress.copper` rose, or no mail, or full price mailed).

- [ ] **Step 3: Change `buy`**

After charging the buyer, **delete** the `seller_online` copper branch. Always:

```rust
mail.deliver_system(
    &listing.seller_durable,
    "Auction House",
    "Auction sold",
    sale_proceeds(listing.price),
    None,
    0,
);
```

Replace buyer `grant_into` with:

```rust
let granted = if let Some(bags) = world.get_mut::<Bags>(buyer) {
    crate::inventory::grant_stack(
        &mut bags.inventory,
        InvStack {
            item_id: listing.item_id.clone(),
            count: listing.count,
            durability: listing.durability,
            enchant_id: listing.enchant_id.clone(),
        },
    )
} else {
    false
};
if !granted {
    events.push(SimEvent::Toast {
        message: "Bags are full.".into(),
    });
    return false;
}
```

Do this **before** deducting copper and removing the listing (same order as today: grant, then pay, then settle, then remove). If grant fails, listing stays.

Update `buy_mails_proceeds_when_seller_offline` expected copper if it asserts `40` on a 40c listing: cut of 40 is 2, proceeds **38**. Change that assertion to `38`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/market.rs
git commit -m "feat(sim): auction house cut and mail-always proceeds"
```

---

### Task 6: Mail attachments + persist + bridge

**Files:**
- Modify: `crates/woc-sim/src/mail.rs`
- Modify: `crates/woc-persist/src/economy.rs`
- Modify: `crates/woc-server/src/bridge.rs`
- Modify: `crates/woc-sim/src/market.rs` (cancel/expire pass instance stacks into mail)

**Interfaces:**
- Consumes: `InvStack`, `grant_stack`, `MarketListingDto` / `MailDto`
- Produces: mail attachments keep durability/enchant; economy JSON omits keys on old rows; cancel/expire mail uses the listed stack

- [ ] **Step 1: Write the failing tests**

In `mail.rs`:

```rust
#[test]
fn collect_restores_listed_wear() {
    let mut box_ = Mailbox::new();
    box_.deliver_system(
        "ada",
        "Auction House",
        "Listing expired",
        0,
        Some(InvStack {
            item_id: "worn_sword".into(),
            count: 1,
            durability: Some(7),
            enchant_id: Some("coarse_sharpening".into()),
        }),
    );
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    let mut events = Vec::new();
    assert!(box_.collect(&mut world, 1, 1, &mut events));
    let sword = world
        .get::<Bags>(1)
        .unwrap()
        .inventory
        .iter()
        .flatten()
        .find(|s| s.item_id == "worn_sword")
        .unwrap();
    assert_eq!(sword.durability, Some(7));
    assert_eq!(sword.enchant_id.as_deref(), Some("coarse_sharpening"));
}
```

This assumes `deliver_system` takes `Option<InvStack>` as the attachment. Update `load_mails_roundtrip` to the new signature (`None` instead of `None, 0`).

In `economy.rs`:

```rust
#[test]
fn economy_omitted_instance_fields_default() {
    let eco: RealmEconomy = serde_json::from_str(
        r#"{"mail":[{"id":1,"from":"AH","to_durable":"ada","subject":"Sold","copper":40,"item_id":null,"item_count":0}],"market":[{"id":2,"seller_durable":"bob","seller_name":"Bob","item_id":"worn_sword","count":1,"price":12,"expires_tick":100}],"next_mail_id":3,"next_listing_id":4}"#,
    )
    .unwrap();
    assert!(eco.mail[0].durability.is_none());
    assert!(eco.market[0].enchant_id.is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-sim collect_restores_listed_wear --offline`

Expected: FAIL compiling (signature / missing fields).

- [ ] **Step 3: Implement**

`MailItem`:

```rust
pub durability: Option<u32>,
pub enchant_id: Option<String>,
```

```rust
pub fn deliver_system(
    &mut self,
    to_durable: &str,
    from: &str,
    subject: &str,
    copper: u32,
    attachment: Option<InvStack>,
) -> u32 {
    let mail_id = self.next_id;
    self.next_id = self.next_id.saturating_add(1);
    let (item_id, item_count, durability, enchant_id) = match attachment {
        Some(stack) => (
            Some(stack.item_id),
            stack.count,
            stack.durability,
            stack.enchant_id,
        ),
        None => (None, 0, None, None),
    };
    self.inbox.entry(to_durable.to_string()).or_default().push(MailItem {
        id: mail_id,
        from: from.to_string(),
        to_durable: to_durable.to_string(),
        subject: subject.to_string(),
        copper,
        item_id,
        item_count,
        durability,
        enchant_id,
    });
    mail_id
}
```

`collect`: when `mail.item_id` is `Some`, build `InvStack` with stored wear (if `durability` is `None` and catalog `max_durability > 0`, use `InvStack::new` then overwrite `enchant_id`; else use stored fields) and `grant_stack`. On failure, put the mail back as today.

`snapshot_for_entity` copies `durability` and `enchant_id` onto `MailSnapshot`.

`send` (player mail) copies the taken bag stack’s durability/enchant onto `MailItem` (use `take_from_slot` when a `bag_slot` is present, instead of `remove_item` by id — same slot bug as AH).

`MailDto` / `MarketListingDto`: additive `#[serde(default)] durability` / `enchant_id`.

`bridge.rs` `apply_economy_to_sim` / `export_economy_from_sim`: copy those four fields. `Listing` load still sets `seller_id: 0`.

Market cancel/expire mail branches:

```rust
mail.deliver_system(
    &listing.seller_durable,
    "Auction House",
    "Listing expired", // or "Listing cancelled"
    0,
    Some(InvStack {
        item_id: listing.item_id,
        count: listing.count,
        durability: listing.durability,
        enchant_id: listing.enchant_id,
    }),
);
```

Bag-first cancel/expire uses `grant_stack` with that same `InvStack`.

Copper-only sale mail: `deliver_system(..., sale_proceeds(price), None)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --offline && cargo test -p woc-persist --offline && cargo test -p woc-server --offline`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/mail.rs crates/woc-sim/src/market.rs crates/woc-persist/src/economy.rs crates/woc-server/src/bridge.rs
git commit -m "feat(sim): persist auction and mail item instances"
```

---

### Task 7: Host-gate list/buy/cancel on auctioneer session

**Files:**
- Modify: `crates/woc-sim/src/host.rs`
- Modify: `crates/woc-sim/src/market.rs` (helper `pub fn auctioneer_session_ok` **or** put the helper in `host.rs` / `interaction.rs`)

**Interfaces:**
- Consumes: `Bags.open_vendor_npc`, `NpcDef::is_auctioneer`, `dist2d`, `INTERACT_RANGE`
- Produces: `WorldHost::interact` market actions toast `"Talk to an auctioneer first."` unless the open session NPC is an in-range auctioneer

- [ ] **Step 1: Write the failing tests**

Put in `market.rs` tests (can construct `Sim`):

```rust
fn npc_id_by_template(world: &World, template: &str) -> EntityId {
    world
        .ids::<Identity>()
        .into_iter()
        .find(|&id| {
            world
                .get::<Identity>(id)
                .and_then(|i| i.template_id.as_deref())
                == Some(template)
        })
        .expect(template)
}

#[test]
fn interact_market_list_requires_auctioneer_session() {
    use woc_protocol::{InteractAction, WorldHost};
    let mut sim = crate::sim::Sim::new_eastbrook("Ada", PlayerClass::Warrior);
    let pid = sim.player_id;
    if let Some(p) = sim.world.get_mut::<Progress>(pid) {
        p.copper = 100;
    }
    if let Some(bags) = sim.world.get_mut::<Bags>(pid) {
        assert!(grant_into(&mut bags.inventory, "silverleaf", 1));
    }
    let slot = sim
        .world
        .get::<Bags>(pid)
        .unwrap()
        .inventory
        .iter()
        .position(|s| s.as_ref().is_some_and(|st| st.item_id == "silverleaf"))
        .unwrap() as u8;
    WorldHost::interact(
        &mut sim,
        pid,
        0,
        InteractAction::MarketList {
            bag_slot: slot,
            count: 1,
            price: 12,
        },
    );
    assert!(sim.market.listings.is_empty());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        SimEvent::Toast { message } if message == "Talk to an auctioneer first."
    )));

    let lise = npc_id_by_template(&sim.world, "auctioneer_lise");
    // Stand next to Lise.
    if let (Some(pt), Some(nt)) = (
        sim.world.get::<crate::ecs::components::Transform>(pid).cloned(),
        sim.world.get::<crate::ecs::components::Transform>(lise).cloned(),
    ) {
        let _ = pt;
        if let Some(p) = sim.world.get_mut::<crate::ecs::components::Transform>(pid) {
            p.x = nt.x;
            p.z = nt.z;
        }
    }
    WorldHost::interact(&mut sim, pid, lise, InteractAction::Talk);
    sim.events.clear();
    WorldHost::interact(
        &mut sim,
        pid,
        lise,
        InteractAction::MarketList {
            bag_slot: slot,
            count: 1,
            price: 12,
        },
    );
    assert_eq!(sim.market.listings.len(), 1);
}
```

Import `Identity`, `EntityId`. Direct `AuctionHouse::list_item` tests must still pass without an NPC.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p woc-sim interact_market_list_requires_auctioneer_session --offline`

Expected: FAIL (list succeeds without Talk).

- [ ] **Step 3: Gate in `host.rs`**

```rust
fn require_auctioneer(
    world: &World,
    player_id: EntityId,
    events: &mut Vec<SimEvent>,
) -> bool {
    use crate::ecs::components::{Bags, Identity};
    use crate::ecs::components::dist2d;
    use crate::types::INTERACT_RANGE;
    use woc_content::npc;
    use woc_protocol::EntityKind;

    let Some(npc_id) = world.get::<Bags>(player_id).and_then(|b| b.open_vendor_npc) else {
        events.push(SimEvent::Toast {
            message: "Talk to an auctioneer first.".into(),
        });
        return false;
    };
    let is_auctioneer = world
        .get::<Identity>(npc_id)
        .and_then(|i| {
            (i.kind == EntityKind::Npc)
                .then(|| i.template_id.as_deref().and_then(npc).map(|d| d.is_auctioneer()))
        })
        .flatten()
        .unwrap_or(false);
    let in_range = dist2d(world, player_id, npc_id)
        .map(|d| d <= INTERACT_RANGE)
        .unwrap_or(false);
    if !is_auctioneer || !in_range {
        events.push(SimEvent::Toast {
            message: "Talk to an auctioneer first.".into(),
        });
        return false;
    }
    true
}
```

Wrap the three market arms:

```rust
InteractAction::MarketList { bag_slot, count, price } => {
    if require_auctioneer(&self.world, player_id, &mut self.events) {
        let _ = self.market.list_item(
            &mut self.world,
            player_id,
            bag_slot,
            count,
            price,
            self.tick,
            &mut self.events,
        );
    }
}
```

Same for `MarketBuy` / `MarketCancel`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-sim --offline`

Expected: PASS. Fingerprint test still `3214741777866168171`.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/host.rs crates/woc-sim/src/market.rs
git commit -m "feat(sim): gate auction actions on Auctioneer Lise"
```

---

### Task 8: Client chrome

**Files:**
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-client/src/nameplates.rs`
- Modify: `crates/woc-client/src/map.rs`
- Modify: `crates/woc-client/src/visuals.rs`
- Modify: `crates/woc-client/src/world_setup.rs` (help footer if it mentions U market)

**Interfaces:**
- Consumes: `can_auction`, listing `durability` / `enchant_id`, `item()` / `enchant()`
- Produces: `[A]` tags; Talk to auctioneer sets `show_market`; **L** lists first non-quest stack; listing lines show name, wear, enchant

- [ ] **Step 1: Write the failing client tests**

In `hud.rs` tests:

```rust
#[test]
fn first_listable_bag_stack_skips_quest_and_allows_weapons() {
    let mut snap = TickSnapshot::default();
    snap.inventory.push(InvSlotSnapshot {
        slot: 0,
        item_id: "boar_tusk".into(),
        count: 1,
        durability: None,
        enchant_id: None,
    });
    snap.inventory.push(InvSlotSnapshot {
        slot: 1,
        item_id: "worn_sword".into(),
        count: 1,
        durability: Some(7),
        enchant_id: Some("coarse_sharpening".into()),
    });
    let listed = first_listable_bag_stack(&snap).unwrap();
    assert_eq!(listed.0, 1);
    assert_eq!(listed.2, "worn_sword");
}

#[test]
fn market_panel_shows_wear_and_enchant() {
    let mut snap = chrome_snapshot();
    snap.market[0].item_id = "worn_sword".into();
    snap.market[0].count = 1;
    snap.market[0].durability = Some(7);
    snap.market[0].enchant_id = Some("coarse_sharpening".into());
    let text = market_panel_text(&snap);
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
    });
    let text = npc_session_help(&snap);
    assert!(text.contains("[U] Auction"));
}
```

Update `market_panel_formats_listings_wallet_and_buy_help` if the peacebloom line format changes (catalog name `Peacebloom` instead of raw id). Use whatever `item("peacebloom").name` is; if the test item id has no catalog row, keep id fallback.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p woc-client first_listable_bag_stack_skips_quest --offline`

Expected: FAIL (`first_listable` still junk-only).

- [ ] **Step 3: Implement client**

`first_listable_bag_stack`:

```rust
pub(crate) fn first_listable_bag_stack(snap: &TickSnapshot) -> Option<(u8, u32, String, u32)> {
    snap.inventory.iter().find_map(|stack| {
        let def = item(&stack.item_id)?;
        if matches!(def.kind, ItemKind::Quest) {
            return None;
        }
        let price = def.vendor_sell.max(1).saturating_mul(5);
        Some((stack.slot, stack.count.min(1).max(1), stack.item_id.clone(), price))
    })
}
```

`market_panel_text` listing line:

```rust
fn listing_line(listing: &MarketListingSnapshot) -> String {
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
    format!(
        "  #{} {}×{name}{extra} — {}c ({}){mine}",
        listing.id, listing.count, listing.price, listing.seller
    )
}
```

Import `enchant` from `woc_content` (hud already imports `item`).

`npc_session_help`: if `open_npc.can_auction`, push `"[U] Auction"`.

`input.rs`: when snapshot `open_npc.can_auction` becomes true this frame, set `ui.show_market = true`. Edge-trigger on `can_auction` going from false→true (compare previous snapshot or set whenever `can_auction && !ui.show_market` on Talk toast — simplest: if `host.snapshot.open_npc.as_ref().is_some_and(|n| n.can_auction) && keys.just_pressed` is not required; in the chrome update / input start:

```rust
if host
    .snapshot
    .open_npc
    .as_ref()
    .is_some_and(|n| n.can_auction)
{
    ui.show_market = true;
}
```

That re-opens U every frame while the session is open, which fights the player toggling U closed. **Do not do that.**

Instead, in `input.rs` where Talk/`KeyE` already runs, after interact, if the *new* snapshot (host may not have ticked yet) — Talk is an interact; the snapshot updates on the next tick. Use a one-shot: when `can_auction` is true and `ui.show_market` was false **and** `InteractAction::Talk` was just sent this frame, set `show_market = true`.

Find the existing **E** interact path and after sending `Talk`:

```rust
// Snapshot still old this frame; set a flag.
ui.open_market_on_auctioneer = true;
```

Then at top of input, if `open_market_on_auctioneer && host.snapshot.open_npc.as_ref().is_some_and(|n| n.can_auction) { ui.show_market = true; ui.open_market_on_auctioneer = false; }`.

Add `open_market_on_auctioneer: bool` to `UiFlags` (default false).

Nameplates / map: `if n.is_auctioneer() { tags.push_str("[A]"); }`

Visuals: add `Cue::Auction` with gold-brown box `Color::srgb(0.92, 0.78, 0.28)` (distinct from quest yellow). Push it when `def.is_auctioneer()`. Copy the Vendor cuboid spawn, change color.

Help footer in `world_setup.rs`: keep `U market (L/O/X)`; no protocol rev bump.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p woc-client --offline && cargo check -p woc-client --offline`

Expected: PASS / check green.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs crates/woc-client/src/input.rs crates/woc-client/src/nameplates.rs crates/woc-client/src/map.rs crates/woc-client/src/visuals.rs crates/woc-client/src/world_setup.rs
git commit -m "feat(client): auctioneer marker and instance listing chrome"
```

---

### Task 9: Docs, STATUS, version tag

**Files:**
- Modify: `docs/parity/STATUS.md`
- Modify: `docs/parity/DEMO.md`
- Modify: `docs/ROADMAP.md` (mark 1.14.0 shipped; remove “planned”)
- Modify: `CHANGELOG.md`
- Modify: `VERSION.toml`
- Modify: `Cargo.toml` workspace.package.version
- Modify: `docs/superpowers/specs/2026-08-13-auction-depth-design.md` status line to Implemented

**Interfaces:**
- Consumes: §6 definition of done
- Produces: rewrite `1.14.0` / `auction-depth`

- [ ] **Step 1: Confirm tests are green before touching version**

Run: `cargo test --workspace --exclude woc-client --offline && cargo check -p woc-client --offline && cargo test -p woc-client --offline`

Expected: all PASS / check green. Fingerprint `3214741777866168171`. Protocol rev 8.

- [ ] **Step 2: STATUS auction-depth table**

Insert after the gear-slots section in `docs/parity/STATUS.md`:

```markdown
**Current rewrite:** `1.14.0` / `auction-depth`.

## Auction depth (`auction-depth`) — done

Design: [`../superpowers/specs/2026-08-13-auction-depth-design.md`](../superpowers/specs/2026-08-13-auction-depth-design.md)
Plan: [`../superpowers/plans/2026-08-13-auction-depth.md`](../superpowers/plans/2026-08-13-auction-depth.md)

| Subsystem | Status | Notes |
| --- | --- | --- |
| Auctioneer Lise | done | Eastbrook `(4, 6)`; Talk opens session; `[A]` |
| Instance listings | done | Slot take; durability + enchant persist |
| Quest block | done | Same toast as vendor sell |
| House cut 5% | done | `price / 20` destroyed; proceeds mailed |
| Mail settlement | done | Online sellers get mail, not silent copper |
| Protocol | done | Rev 8 additive fields |
```

Change the completion-table “Auction market” notes to mention instance fidelity + auctioneer.

DEMO step 6 becomes:

```text
6. Bank an item and copper; mail copper; Talk to Auctioneer Lise [A], list then buy/cancel on the AH (wear/enchant survive); gather + craft a salve or copper shortsword.
```

Footer `WoC-rs 1.14.0`. ROADMAP 1.14.0 row `(shipped)`.

CHANGELOG:

```markdown
## 1.14.0 — 2026-08-13

### Added

- **1.14.0 `auction-depth`:** Eastbrook Auctioneer Lise; list/buy/cancel require her session.
- Auction listings keep durability and enchant; 5% house cut; sale proceeds always arrive as mail.
- Quest items cannot be listed. Client **L** lists the first non-quest bag stack; `[A]` nameplate.
- Protocol rev stays **8** (additive listing/mail/`can_auction` fields).
```

`VERSION.toml`: `rewrite_version = "1.14.0"`, `parity_target = "auction-depth"`.

Workspace `Cargo.toml`: `version = "1.14.0"`.

Spec header: `**Status:** Implemented (1.14.0).`

- [ ] **Step 3: Run version-sensitive tests if any assert `1.13.0`**

Run: `rg "1\\.13\\.0" crates docs VERSION.toml Cargo.toml -g '!**/target/**'`

Update crate tests that lock the rewrite version string (search `rewrite_version` / `"1.13.0"` in `crates/`).

Run: `cargo test --workspace --exclude woc-client --offline`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add docs VERSION.toml Cargo.toml crates
git commit -m "docs: ship auction-depth as 1.14.0"
```

---

## Self-review (plan vs spec)

| Spec § | Task |
| --- | --- |
| 5.1 Auctioneer Lise | Task 1 |
| 5.2 take_from_slot / grant_stack | Task 2 |
| 5.3 Listing payload, quest block, slot take | Task 4 |
| 5.4 House cut, mail-always | Task 5 |
| 5.5 Mail instance | Task 6 |
| 5.6 Host gate | Task 7 |
| 5.7 Protocol | Task 3 |
| 5.8 Persist / bridge | Task 6 |
| 5.9 Client | Task 8 |
| §6 DoD + docs | Task 9 |
| Non-goals (bids, banker NPC, rev 9) | no task — omitted |

No TBD/TODO placeholders. `sale_cut(50) == 2`, proceeds 48. Enchant id on gear is `coarse_sharpening` (oil item remains `coarse_whetstone`). Fingerprint `3214741777866168171`. `PROTOCOL_REV` 8.
