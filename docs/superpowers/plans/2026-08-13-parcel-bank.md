# Parcel and Bank (Warehouse) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship rewrite `1.17.0` / `parcel-bank`: slot-accurate bank and mail moves that preserve durability/enchant, offline parcels via a realm character directory, client send/collect/return, postage/inbox cap/expiry, and repair that includes banked gear.

**Architecture:** `take_from_slot` / `put_stack` in `inventory.rs` become the only economy *move* path. Bank stays the player `Bank` column. Mailbox stays a `Sim` resource. `CharacterDirectory` is a new `Sim` field (not a World column). Mail expiry hooks inside `pvp_and_market`. Protocol rev stays 8 with serde defaults. HUD **K**/**I** stay ungated.

**Tech Stack:** Rust 2021, existing crates, Bevy 0.16 client, protocol rev 8, upstream 0.31.0.

**Design spec:** `docs/superpowers/specs/2026-08-13-parcel-bank-design.md`

## Global Constraints

- Upstream pin remains `0.31.0` / `a3e5e9596a8e9e7d37b5b23efbbb0f2cd846c0c9`.
- `PROTOCOL_REV` remains **8**. New fields `#[serde(default)]`.
- `woc-sim` and `woc-content` must not depend on Bevy, wgpu, axum, or tokio.
- Bank stays on the player `Bank` column. Mailbox + `CharacterDirectory` stay `Sim` fields. Do not add a fat `Entity`.
- Tick-phase fingerprint stays `3214741777866168171u64`. No new named phase. Mail expiry is a call inside `pvp_and_market`.
- English-only toasts. Locked copy: `"Bank is full."`, `"Bags are full."`, `"Recipient not found."`, `"Cannot mail yourself."`, `"Mail is empty."`, `"This item is needed for a quest."`, `"Not enough copper."`, `"Mailbox is full."`, `"Mail returned."`, `"Mail discarded."`, `"Mail not found."`, `"Empty bag slot."`, `"Empty bank slot."`
- Postage `1`, inbox cap `20`, TTL `1_728_000` ticks.
- `grant_into` / `remove_item` stay for loot/craft/quest/vendor **new** grants. Bank, mail, and AH *moves* use slot helpers.
- Author for commits: `yoefun <xinglinsky@outlook.com>`.
- Do not bump `VERSION.toml` / workspace version until the implementation wave is ready to tag `1.14.0`.

## File map

- `crates/woc-sim/src/inventory.rs` — `take_from_slot`, `put_stack`
- `crates/woc-sim/src/bank.rs` — slot moves
- `crates/woc-sim/src/interaction.rs` — `repair_cost` / `repair_all` include `Bank`
- `crates/woc-protocol/src/lib.rs` — `MailReturn`, snapshot fields, `mail_postage`
- `crates/woc-sim/src/mail.rs` — directory types live here or `directory.rs`; send/collect/return/expire
- `crates/woc-sim/src/sim.rs` — `directory` field, register on spawn, snapshot `mail_postage`, `tick_expire`
- `crates/woc-sim/src/host.rs` — pass `now_tick`; route `MailReturn`
- `crates/woc-sim/src/market.rs` — listing instance fields; `take_from_slot` / `deliver_system` attachment
- `crates/woc-persist/src/economy.rs` — DTO fields
- `crates/woc-persist/src/{lib,memory,postgres}.rs` — `list_mailbox_directory`
- `crates/woc-server/src/{bridge,game_ws}.rs` — DTO map + boot directory
- `crates/woc-client/src/{hud,input}.rs` — bank any non-quest; mail compose/send/collect/return
- `docs/parity/{STATUS,DEMO}.md`, `docs/ROADMAP.md`, `CHANGELOG.md`, `VERSION.toml` — tag wave

---

### Task 1: Slot-accurate stack moves

**Files:**
- Modify: `crates/woc-sim/src/inventory.rs`

**Interfaces:**
- Consumes: `InvStack`, existing `grant_into` stack-size rules (`ItemKind::Weapon | Armor` → max 1)
- Produces: `take_from_slot(inv, slot, count) -> Option<InvStack>`, `put_stack(inv, stack) -> Result<(), InvStack>`

- [ ] **Step 1: Write the failing tests** at the bottom of `inventory.rs`:

```rust
#[test]
fn take_from_slot_keeps_wear_and_enchant() {
    let mut inv = vec![None; 4];
    inv[1] = Some(InvStack {
        item_id: "worn_sword".into(),
        count: 1,
        durability: Some(12),
        enchant_id: Some("coarse_sharpening".into()),
    });
    let taken = take_from_slot(&mut inv, 1, 1).unwrap();
    assert_eq!(taken.durability, Some(12));
    assert_eq!(taken.enchant_id.as_deref(), Some("coarse_sharpening"));
    assert!(inv[1].is_none());
}

#[test]
fn take_from_slot_splits_stackable() {
    let mut inv = vec![None; 4];
    inv[0] = Some(InvStack::new("silverleaf", 5));
    let taken = take_from_slot(&mut inv, 0, 2).unwrap();
    assert_eq!(taken.count, 2);
    assert_eq!(inv[0].as_ref().unwrap().count, 3);
}

#[test]
fn put_stack_merges_matching_and_rejects_full() {
    let mut inv = vec![None; 1];
    assert!(put_stack(&mut inv, InvStack::new("silverleaf", 5)).is_ok());
    assert!(put_stack(&mut inv, InvStack::new("silverleaf", 3)).is_ok());
    assert_eq!(inv[0].as_ref().unwrap().count, 8);
    let err = put_stack(&mut inv, InvStack::new("wolf_fang", 1)).unwrap_err();
    assert_eq!(err.item_id, "wolf_fang");
}

#[test]
fn put_stack_does_not_merge_mismatched_enchant() {
    let mut inv = vec![None; 2];
    let a = InvStack {
        item_id: "worn_sword".into(),
        count: 1,
        durability: Some(12),
        enchant_id: Some("coarse_sharpening".into()),
    };
    let b = InvStack {
        item_id: "worn_sword".into(),
        count: 1,
        durability: Some(12),
        enchant_id: None,
    };
    assert!(put_stack(&mut inv, a).is_ok());
    assert!(put_stack(&mut inv, b).is_ok());
    assert!(inv[0].is_some() && inv[1].is_some());
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim take_from_slot_keeps_wear_and_enchant -- --nocapture`

Expected: FAIL (unresolved `take_from_slot`)

- [ ] **Step 3: Implement** in `inventory.rs`:

```rust
pub fn take_from_slot(
    inv: &mut [Option<InvStack>],
    slot: usize,
    count: u32,
) -> Option<InvStack> {
    let stack = inv.get_mut(slot)?.as_mut()?;
    let take = count.min(stack.count).max(1);
    if take < stack.count {
        stack.count -= take;
        let mut taken = stack.clone();
        taken.count = take;
        Some(taken)
    } else {
        inv[slot].take()
    }
}

fn max_stack_for(item_id: &str) -> u32 {
    let stack_size = woc_content::item(item_id)
        .map(|d| d.stack_size.max(1))
        .unwrap_or(20);
    let unstacked = woc_content::item(item_id)
        .map(|d| matches!(d.kind, ItemKind::Weapon | ItemKind::Armor))
        .unwrap_or(false);
    if unstacked {
        1
    } else {
        stack_size
    }
}

fn stacks_merge(a: &InvStack, b: &InvStack) -> bool {
    a.item_id == b.item_id && a.durability == b.durability && a.enchant_id == b.enchant_id
}

pub fn put_stack(inv: &mut [Option<InvStack>], mut stack: InvStack) -> Result<(), InvStack> {
    if stack.count == 0 {
        return Ok(());
    }
    let max_stack = max_stack_for(&stack.item_id);
    if max_stack > 1 {
        for slot in inv.iter_mut().flatten() {
            if stacks_merge(slot, &stack) && slot.count < max_stack {
                let space = max_stack - slot.count;
                let add = stack.count.min(space);
                slot.count += add;
                stack.count -= add;
                if stack.count == 0 {
                    return Ok(());
                }
            }
        }
    }
    while stack.count > 0 {
        let Some(empty) = inv.iter_mut().find(|s| s.is_none()) else {
            return Err(stack);
        };
        let add = stack.count.min(max_stack);
        let mut placed = stack.clone();
        placed.count = add;
        stack.count -= add;
        *empty = Some(placed);
    }
    Ok(())
}
```

- [ ] **Step 4: Run** `cargo test -p woc-sim --lib inventory -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/inventory.rs
git commit -m "feat(sim): take_from_slot and put_stack preserve item instances"
```

---

### Task 2: Bank deposit/withdraw use slot moves; repair includes warehouse

**Files:**
- Modify: `crates/woc-sim/src/bank.rs`
- Modify: `crates/woc-sim/src/interaction.rs` (`repair_cost`, `repair_all`)
- Test: `crates/woc-sim/src/bank.rs`, `crates/woc-sim/src/sim.rs` (extend `repair_all_at_smith_restores_gear` or add a sibling)

**Interfaces:**
- Consumes: `take_from_slot`, `put_stack`
- Produces: bank deposit/withdraw that copy durability/enchant; `repair_cost` sums `Bank.bank`

- [ ] **Step 1: Failing bank test** in `bank.rs`:

```rust
#[test]
fn deposit_preserves_worn_enchanted_sword() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    let slot = world
        .get::<Bags>(1)
        .unwrap()
        .inventory
        .iter()
        .position(|s| s.as_ref().is_some_and(|x| x.item_id == "worn_sword"))
        .unwrap();
    if let Some(bags) = world.get_mut::<Bags>(1) {
        if let Some(st) = bags.inventory[slot].as_mut() {
            st.durability = Some(12);
            st.enchant_id = Some("coarse_sharpening".into());
        }
    }
    let mut events = Vec::new();
    assert!(deposit(&mut world, 1, slot as u8, 1, &mut events));
    let stored = world
        .get::<Bank>(1)
        .unwrap()
        .bank
        .iter()
        .flatten()
        .find(|s| s.item_id == "worn_sword")
        .unwrap();
    assert_eq!(stored.durability, Some(12));
    assert_eq!(stored.enchant_id.as_deref(), Some("coarse_sharpening"));
}
```

- [ ] **Step 2: Run** `cargo test -p woc-sim deposit_preserves_worn_enchanted_sword -- --nocapture`

Expected: FAIL (`durability` is `Some(40)` from `InvStack::new`)

- [ ] **Step 3: Rewrite `deposit` / `withdraw`** to take/put the slot instead of `remove_item`/`grant_into`:

```rust
pub fn deposit(
    world: &mut World,
    player_id: EntityId,
    bag_slot: u8,
    count: u32,
    events: &mut Vec<SimEvent>,
) -> bool {
    ensure_bank(world, player_id);
    let Some(taken) = world.get_mut::<Bags>(player_id).and_then(|b| {
        crate::inventory::take_from_slot(&mut b.inventory, bag_slot as usize, count)
    }) else {
        events.push(SimEvent::Toast {
            message: "Empty bag slot.".into(),
        });
        return false;
    };
    let item_id = taken.item_id.clone();
    let n = taken.count;
    let bank_full = match world.get_mut::<Bank>(player_id) {
        Some(bank) => crate::inventory::put_stack(&mut bank.bank, taken.clone()).is_err(),
        None => true,
    };
    if bank_full {
        if let Some(bags) = world.get_mut::<Bags>(player_id) {
            let _ = crate::inventory::put_stack(&mut bags.inventory, taken);
        }
        events.push(SimEvent::Toast {
            message: "Bank is full.".into(),
        });
        return false;
    }
    events.push(SimEvent::ItemLost {
        player: player_id,
        item_id,
        count: n,
    });
    true
}
```

Mirror for `withdraw` (take bank slot, `put_stack` bags, rollback on `Err`). Keep `ensure_bank` as a small helper extracted from today's insert/resize block.

In `repair_cost`, after the bag loop:

```rust
if let Some(bank) = world.get::<Bank>(player_id) {
    for stack in bank.bank.iter().flatten() {
        let Some(def) = item(&stack.item_id) else {
            continue;
        };
        if def.max_durability == 0 {
            continue;
        }
        let current = stack.durability.unwrap_or(def.max_durability);
        cost = cost.saturating_add(def.max_durability.saturating_sub(current));
    }
}
```

In `repair_all`, after bag restores, restore every `Bank.bank` gear stack to `max_durability`.

Add test `repair_cost_includes_banked_gear` that puts a 0-durability `worn_sword` in bank and asserts `repair_cost == 40` (plus any equipped wear from spawn kit — subtract equipped/bag first, or unequip/clear bags in the test).

- [ ] **Step 4: Run** `cargo test -p woc-sim --lib bank -- --nocapture` and `cargo test -p woc-sim repair_all_at_smith_restores_gear repair_cost_includes_banked_gear -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/bank.rs crates/woc-sim/src/interaction.rs
git commit -m "feat(sim): bank moves preserve instances; repair includes warehouse"
```

---

### Task 3: Protocol — MailReturn, snapshot instance fields, postage

**Files:**
- Modify: `crates/woc-protocol/src/lib.rs`

**Interfaces:**
- Consumes: existing tagged `InteractAction`, `MailSnapshot`, `TickSnapshot`
- Produces: `InteractAction::MailReturn { mail_id: u32 }`; `MailSnapshot.{durability,enchant_id,expires_tick}`; `TickSnapshot.mail_postage: u32`

- [ ] **Step 1: Failing tests** next to the existing protocol tests:

```rust
#[test]
fn mail_return_roundtrip() {
    let a = InteractAction::MailReturn { mail_id: 9 };
    let back: InteractAction = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    assert!(matches!(back, InteractAction::MailReturn { mail_id: 9 }));
}

#[test]
fn mail_snapshot_omitted_instance_fields_default() {
    let mail: MailSnapshot = serde_json::from_str(
        r#"{"id":1,"from":"AH","subject":"Sold","copper":40,"item_count":0}"#,
    )
    .unwrap();
    assert!(mail.durability.is_none());
    assert!(mail.enchant_id.is_none());
    assert_eq!(mail.expires_tick, 0);
}

#[test]
fn tick_snapshot_mail_postage_defaults_zero() {
    let snap: TickSnapshot = serde_json::from_str(minimal_tick_json()).unwrap();
    assert_eq!(snap.mail_postage, 0);
    assert_eq!(PROTOCOL_REV, 8);
}
```

- [ ] **Step 2: Run** `cargo test -p woc-protocol mail_return_roundtrip -- --nocapture`

Expected: FAIL (unknown variant)

- [ ] **Step 3: Implement**

Add to `InteractAction` after `MailCollect`:

```rust
MailReturn { mail_id: u32 },
```

Add to `MailSnapshot`:

```rust
#[serde(default)]
pub durability: Option<u32>,
#[serde(default)]
pub enchant_id: Option<String>,
#[serde(default)]
pub expires_tick: u64,
```

Add to `TickSnapshot` (end of struct):

```rust
/// Postage in copper the sim will charge for player-to-player mail.
#[serde(default)]
pub mail_postage: u32,
```

Update every `TickSnapshot { ... }` struct literal in this file to include `mail_postage: 0`. Update `MailSnapshot { ... }` literals to omit the new fields (defaults) or set them explicitly. Extend the existing action-roundtrip test that already lists `MailCollect` so it also serializes `MailReturn`.

- [ ] **Step 4: Run** `cargo test -p woc-protocol -- --nocapture`

Expected: PASS (`PROTOCOL_REV` still 8)

- [ ] **Step 5: Commit**

```bash
git add crates/woc-protocol/src/lib.rs
git commit -m "feat(protocol): MailReturn and additive mail instance snapshot fields"
```

---

### Task 4: CharacterDirectory + parcel send/collect/return/expire

**Files:**
- Modify: `crates/woc-sim/src/mail.rs`
- Modify: `crates/woc-sim/src/sim.rs` (struct field + spawn register; tick call can wait for Task 7)
- Modify: `crates/woc-sim/src/lib.rs` if you add `pub use mail::{CharacterDirectory, MAIL_POSTAGE, ...}`

**Interfaces:**
- Consumes: `take_from_slot`, `put_stack`, `InteractAction` not required here
- Produces: `CharacterDirectory`, `MAIL_POSTAGE`, `MAIL_INBOX_CAP`, `MAIL_TTL_TICKS`, `MailAttachment`, `Mailbox::send(..., now_tick, directory)`, `return_mail`, `tick_expire`, `deliver_system(..., attachment)`

- [ ] **Step 1: Failing tests** in `mail.rs`:

```rust
#[test]
fn send_offline_via_directory_preserves_instance() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 10;
    }
    let slot = world
        .get::<Bags>(1)
        .unwrap()
        .inventory
        .iter()
        .position(|s| s.as_ref().is_some_and(|x| x.item_id == "worn_sword"))
        .unwrap();
    if let Some(bags) = world.get_mut::<Bags>(1) {
        if let Some(st) = bags.inventory[slot].as_mut() {
            st.durability = Some(12);
            st.enchant_id = Some("coarse_sharpening".into());
        }
    }
    let mut dir = CharacterDirectory::default();
    dir.register("Bob", "bob");
    let mut box_ = Mailbox::new();
    let mut events = Vec::new();
    assert!(box_.send(
        &mut world,
        1,
        "Bob",
        0,
        Some(slot as u8),
        1,
        0,
        &dir,
        &mut events,
    ));
    assert_eq!(world.get::<Progress>(1).unwrap().copper, 9); // postage
    crate::ecs::spawn::create_player(&mut world, 99, "Bob", PlayerClass::Mage, 1.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(99) {
        d.durable_id = Some("bob".into());
    }
    assert!(box_.collect(&mut world, 99, 1, &mut events));
    let got = world
        .get::<Bags>(99)
        .unwrap()
        .inventory
        .iter()
        .flatten()
        .find(|s| s.item_id == "worn_sword")
        .unwrap();
    assert_eq!(got.durability, Some(12));
    assert_eq!(got.enchant_id.as_deref(), Some("coarse_sharpening"));
}

#[test]
fn player_mail_expires_and_returns() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    crate::ecs::spawn::create_player(&mut world, 2, "Bob", PlayerClass::Mage, 1.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    if let Some(d) = world.get_mut::<Durable>(2) {
        d.durable_id = Some("bob".into());
    }
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 5;
    }
    let mut dir = CharacterDirectory::default();
    dir.register("Bob", "bob");
    let mut box_ = Mailbox::new();
    let mut events = Vec::new();
    assert!(box_.send(&mut world, 1, "Bob", 2, None, 0, 0, &dir, &mut events));
    box_.tick_expire(MAIL_TTL_TICKS, &mut events);
    assert!(box_.snapshot_for_entity(2, &world).is_empty());
    let returned = &box_.snapshot_for_entity(1, &world)[0];
    assert_eq!(returned.subject, "Returned: Parcel");
    assert_eq!(returned.copper, 2);
}

#[test]
fn inbox_cap_blocks_player_mail_not_system() {
    let mut box_ = Mailbox::new();
    for _ in 0..MAIL_INBOX_CAP {
        box_.deliver_system("bob", "Ada", "Parcel", 1, None);
    }
    // player cap counts all mails in the inbox including system
    // Spec: system bypasses cap on deliver_system; player send checks len >= cap
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 50;
    }
    let mut dir = CharacterDirectory::default();
    dir.register("Bob", "bob");
    let mut events = Vec::new();
    assert!(!box_.send(&mut world, 1, "Bob", 1, None, 0, 0, &dir, &mut events));
    assert!(events.iter().any(|e| matches!(e, SimEvent::Toast { message } if message == "Mailbox is full.")));
    box_.deliver_system("bob", "Auction House", "Sold", 40, None);
    assert_eq!(box_.snapshot_for_entity_key("bob").len(), MAIL_INBOX_CAP as usize + 1);
}
```

If `snapshot_for_entity_key` is awkward, assert via `all_mails().iter().filter(|m| m.to_durable == "bob").count()`.

Also keep `send_and_collect_copper_and_item` compiling by updating its `send` call with `now_tick` + `&dir` (register Bob even though he is online).

- [ ] **Step 2: Run** `cargo test -p woc-sim send_offline_via_directory_preserves_instance -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement** in `mail.rs` (signatures locked here for later tasks):

```rust
pub const MAIL_POSTAGE: u32 = 1;
pub const MAIL_INBOX_CAP: usize = 20;
pub const MAIL_TTL_TICKS: u64 = 1_728_000;

#[derive(Debug, Clone, Default)]
pub struct CharacterDirectory {
    by_name: std::collections::HashMap<String, String>,
}

impl CharacterDirectory {
    pub fn register(&mut self, name: &str, durable_key: impl Into<String>) {
        self.by_name.insert(name.to_ascii_lowercase(), durable_key.into());
    }
    pub fn lookup(&self, name: &str) -> Option<&str> {
        self.by_name.get(&name.to_ascii_lowercase()).map(String::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct MailAttachment {
    pub item_id: String,
    pub count: u32,
    pub durability: Option<u32>,
    pub enchant_id: Option<String>,
}
```

Extend `MailItem` with `durability`, `enchant_id`, `expires_tick`, `return_to: Option<String>`.

`send` signature:

```rust
pub fn send(
    &mut self,
    world: &mut World,
    from: EntityId,
    to_name: &str,
    copper: u32,
    bag_slot: Option<u8>,
    count: u32,
    now_tick: u64,
    directory: &CharacterDirectory,
    events: &mut Vec<SimEvent>,
) -> bool
```

Resolution: `directory.lookup(to_name)` else live `ClassKit` name scan else toast `"Recipient not found."`

Self: compare `mailbox_key(world, from)` to resolved key.

Empty: `bag_slot.is_none() && copper == 0` → `"Mail is empty."`

Quest: if bag slot's item `ItemKind::Quest` → `"This item is needed for a quest."` (look up before take).

Postage: `wallet < MAIL_POSTAGE + copper` → `"Not enough copper."`

Cap: `self.inbox.get(to_key).map(|v| v.len()).unwrap_or(0) >= MAIL_INBOX_CAP` → `"Mailbox is full."`

Then `take_from_slot`, subtract `MAIL_POSTAGE + copper`, push item with `subject: "Parcel"`, `expires_tick: now_tick.saturating_add(MAIL_TTL_TICKS)`, `return_to: Some(sender_key)`.

`collect` uses `put_stack` with an `InvStack` built from the mail's instance fields (not `grant_into`).

```rust
pub fn deliver_system(
    &mut self,
    to_durable: &str,
    from: &str,
    subject: &str,
    copper: u32,
    attachment: Option<MailAttachment>,
) -> u32
```

Sets `expires_tick = 0`, `return_to = None`. Does not check cap.

```rust
pub fn return_mail(&mut self, world: &mut World, player: EntityId, mail_id: u32, events: &mut Vec<SimEvent>) -> bool
```

Remove mail; if `return_to` Some, `deliver_system` with subject `format!("Returned: {old}")` and toast `"Mail returned."`; else toast `"Mail discarded."`

```rust
pub fn tick_expire(&mut self, now_tick: u64, events: &mut Vec<SimEvent>)
```

Drain mails where `expires_tick > 0 && now_tick >= expires_tick` and return them as system mail to `return_to`. Ignore `events` or leave unused (`let _ = events`).

On `Sim`:

```rust
pub directory: crate::mail::CharacterDirectory,
```

Init `CharacterDirectory::default()` in `new_empty_eastbrook`. At the end of `spawn_player` and `spawn_player_with_state` (after the player exists):

```rust
let key = crate::mail::Mailbox::mailbox_key(&self.world, id);
self.directory.register(name, key);
```

- [ ] **Step 4: Run** `cargo test -p woc-sim --lib mail -- --nocapture`

Expected: FAIL on `market.rs` / `host.rs` until Task 5–7 compile. If the crate does not compile, land `deliver_system` call-site updates in the same commit as Task 5 rather than leaving a red tree. Prefer finishing Task 4 types then immediately Task 5 in the same sitting if `cargo test -p woc-sim --lib mail` cannot compile.

If `market.rs` still calls the old `deliver_system`, temporarily keep a wrapper:

```rust
pub fn deliver_system(
    &mut self,
    to_durable: &str,
    from: &str,
    subject: &str,
    copper: u32,
    item_id: Option<String>,
    item_count: u32,
) -> u32 {
    let attachment = item_id.map(|item_id| MailAttachment {
        count: item_count.max(1),
        durability: None,
        enchant_id: None,
        item_id,
    });
    self.deliver_system_ex(to_durable, from, subject, copper, attachment)
}
```

Name the new method `deliver_system` and update call sites in Task 5. For Task 4, add the extra args with defaults via the wrapper so `mail` tests pass **and** `cargo test -p woc-sim --lib mail` compiles. Updating `host.rs` `send` to pass `self.tick` and `&self.directory` is required for compile; do that in this task (one extra file) — it is not behavior-complete until expiry is ticked.

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/mail.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/host.rs
git commit -m "feat(sim): offline parcels, postage, cap, return, and expiry data"
```

---

### Task 5: Persist DTOs + mailbox directory API + bridge

**Files:**
- Modify: `crates/woc-persist/src/economy.rs`
- Modify: `crates/woc-persist/src/memory.rs`
- Modify: `crates/woc-persist/src/postgres.rs`
- Modify: `crates/woc-persist/src/lib.rs`
- Modify: `crates/woc-server/src/bridge.rs`

**Interfaces:**
- Consumes: `MailItem` new fields, `Listing` instance fields (Listing fields can default until Task 6)
- Produces: `MailDto` additive fields; `Persist::list_mailbox_directory`; bridge maps durability/enchant/expires/return_to; `deliver_system` attachment through `MailItem`

- [ ] **Step 1: Failing persist tests**

In `economy.rs`:

```rust
#[test]
fn mail_dto_omitted_keys_default() {
    let m: MailDto = serde_json::from_str(
        r#"{"id":1,"from":"AH","to_durable":"ada","subject":"Sold","copper":40,"item_count":0}"#,
    )
    .unwrap();
    assert!(m.durability.is_none());
    assert!(m.enchant_id.is_none());
    assert_eq!(m.expires_tick, 0);
    assert!(m.return_to.is_none());
}
```

In `memory.rs` tests:

```rust
#[tokio::test]
async fn list_mailbox_directory_returns_created_names() {
    let store = MemoryStore::new();
    let (aid, _) = store.register("hero_one", "secret1").await.unwrap();
    let c = store.create_character(aid, "Aldric", "warrior").await.unwrap();
    let dir = store.list_mailbox_directory().await.unwrap();
    assert!(dir.iter().any(|(n, id)| n == "Aldric" && *id == c.id));
}
```

- [ ] **Step 2: Run** `cargo test -p woc-persist mail_dto_omitted_keys_default -- --nocapture`

Expected: FAIL (missing fields)

- [ ] **Step 3: Implement**

`MailDto`:

```rust
#[serde(default)]
pub durability: Option<u32>,
#[serde(default)]
pub enchant_id: Option<String>,
#[serde(default)]
pub expires_tick: u64,
#[serde(default)]
pub return_to: Option<String>,
```

`MarketListingDto` (needed by Task 6; add now so bridge compiles once):

```rust
#[serde(default)]
pub durability: Option<u32>,
#[serde(default)]
pub enchant_id: Option<String>,
```

`MemoryStore::list_mailbox_directory`:

```rust
pub async fn list_mailbox_directory(&self) -> PersistResult<Vec<(String, Uuid)>> {
    let g = self.inner.lock().expect("memory store lock");
    Ok(g.characters.values().map(|c| (c.name.clone(), c.id)).collect())
}
```

`PostgresStore::list_mailbox_directory`:

```rust
pub async fn list_mailbox_directory(&self) -> PersistResult<Vec<(String, Uuid)>> {
    let rows = sqlx::query("SELECT name, id FROM characters")
        .fetch_all(&self.pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get::<String, _>("name"), row.get::<Uuid, _>("id")))
        .collect())
}
```

`Persist::list_mailbox_directory` match-dispatch like `list_characters`.

Bridge `apply_economy_to_sim` / `export_economy_from_sim`: copy the new mail fields. Listing durability/enchant: map `None` until Task 6 adds them on `Listing` — if `Listing` lacks fields, keep `None` on export and ignore on import. Prefer adding the `Listing` fields in Task 6 immediately after if this does not compile.

Update `economy_roundtrip` literal to still compile (new fields default).

- [ ] **Step 4: Run** `cargo test -p woc-persist -- --nocapture` and `cargo test -p woc-server -- --nocapture`

Expected: PASS (postgres tests skip without `DATABASE_URL`)

- [ ] **Step 5: Commit**

```bash
git add crates/woc-persist crates/woc-server/src/bridge.rs
git commit -m "feat(persist): mail instance fields and mailbox directory listing"
```

---

### Task 6: Auction listings preserve instances

**Files:**
- Modify: `crates/woc-sim/src/market.rs`
- Modify: `crates/woc-server/src/bridge.rs` (listing field map if not done)

**Interfaces:**
- Consumes: `take_from_slot`, `put_stack`, `MailAttachment`, `ItemKind::Quest`
- Produces: `Listing.{durability,enchant_id}`; list/cancel/expire/buy use instance moves

- [ ] **Step 1: Failing test** in `market.rs` tests:

```rust
#[test]
fn list_and_cancel_returns_worn_enchant() {
    let mut world = World::new();
    crate::ecs::spawn::create_player(&mut world, 1, "Ada", PlayerClass::Warrior, 0.0, 0.0);
    if let Some(d) = world.get_mut::<Durable>(1) {
        d.durable_id = Some("ada".into());
    }
    if let Some(p) = world.get_mut::<Progress>(1) {
        p.copper = 20;
    }
    let slot = world
        .get::<Bags>(1)
        .unwrap()
        .inventory
        .iter()
        .position(|s| s.as_ref().is_some_and(|x| x.item_id == "worn_sword"))
        .unwrap();
    if let Some(bags) = world.get_mut::<Bags>(1) {
        if let Some(st) = bags.inventory[slot].as_mut() {
            st.durability = Some(12);
            st.enchant_id = Some("coarse_sharpening".into());
        }
    }
    let mut ah = AuctionHouse::new();
    let mut mail = Mailbox::new();
    let mut events = Vec::new();
    assert!(ah.list_item(&mut world, 1, slot as u8, 1, 10, 0, &mut events));
    assert!(ah.cancel(&mut world, &mut mail, 1, 1, &mut events));
    let mail_item = mail.all_mails().into_iter().next().unwrap();
    assert_eq!(mail_item.durability, Some(12));
    assert_eq!(mail_item.enchant_id.as_deref(), Some("coarse_sharpening"));
}
```

Inspect `cancel` — if cancel currently `grant_into` bags when seller is online, the test should instead assert the bag stack after cancel **or** the mail, matching current cancel behavior. Preserve that branch, but pass instance fields through both `put_stack` (online) and `deliver_system` (offline/full).

- [ ] **Step 2: Run** `cargo test -p woc-sim list_and_cancel_returns_worn_enchant -- --nocapture`

Expected: FAIL

- [ ] **Step 3: Implement**

Add to `Listing`: `durability: Option<u32>`, `enchant_id: Option<String>`.

`list_item`: refuse quest (`"This item is needed for a quest."`); `take_from_slot` instead of `remove_item`; store wear/enchant on the listing.

Every `deliver_system(...)` call that returns an item passes `Some(MailAttachment { item_id, count, durability: listing.durability.clone(), enchant_id: listing.enchant_id.clone() })`. Online `grant_into` becomes `put_stack` of that `InvStack`.

Bridge maps the two listing fields both ways.

- [ ] **Step 4: Run** `cargo test -p woc-sim --lib market -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/market.rs crates/woc-server/src/bridge.rs
git commit -m "feat(sim): auction listings preserve durability and enchants"
```

---

### Task 7: Host routing, snapshot postage, expiry tick, quest mail block

**Files:**
- Modify: `crates/woc-sim/src/host.rs`
- Modify: `crates/woc-sim/src/sim.rs` (`tick_all` phase 7 + snapshot `mail_postage`)
- Modify: `crates/woc-sim/src/mail.rs` (`snapshot_for_entity` copies new fields)

**Interfaces:**
- Consumes: `Mailbox::send` new args, `return_mail`, `tick_expire`, `MAIL_POSTAGE`
- Produces: wired actions; fingerprint **unchanged**

- [ ] **Step 1: Failing fingerprint-adjacent test** in `sim.rs`:

```rust
#[test]
fn mail_expiry_runs_inside_pvp_and_market_without_fingerprint_change() {
    assert_eq!(tick_phase_fingerprint(), 3214741777866168171u64);
    let mut sim = Sim::new_eastbrook("Ada", PlayerClass::Warrior);
    let bob = sim.spawn_player("Bob", PlayerClass::Mage).unwrap();
    if let Some(p) = sim.world.get_mut::<Progress>(sim.player_id) {
        p.copper = 5;
    }
    sim.interact(0, InteractAction::MailSend {
        to_name: "Bob".into(),
        copper: 2,
        bag_slot: None,
        count: 0,
    });
    assert_eq!(sim.snapshot_for(bob).mail.len(), 1);
    sim.tick = MAIL_TTL_TICKS;
    sim.tick_all();
    assert!(sim.snapshot_for(bob).mail.is_empty());
    assert_eq!(sim.snapshot_for(sim.player_id).mail[0].subject, "Returned: Parcel");
    assert_eq!(sim.snapshot_for(sim.player_id).mail_postage, MAIL_POSTAGE);
}
```

Use the real `WorldHost::interact` signature (`player_id`, `target_id`, `action`). Match existing tests in `sim.rs` (`sim.interact(smith, InteractAction::RepairAll)`).

- [ ] **Step 2: Run** `cargo test -p woc-sim mail_expiry_runs_inside_pvp_and_market_without_fingerprint_change -- --nocapture`

Expected: FAIL (mail still in Bob's inbox)

- [ ] **Step 3: Implement**

`host.rs` `MailSend`: pass `self.tick`, `&self.directory`. Add:

```rust
InteractAction::MailReturn { mail_id } => {
    let _ = self.mail.return_mail(&mut self.world, player_id, mail_id, &mut self.events);
}
```

`sim.rs` phase 7 after `market.tick_expire`:

```rust
self.mail.tick_expire(self.tick, &mut self.events);
```

Snapshot: `mail_postage: if world.get::<ClassKit>(player_id).is_some() { MAIL_POSTAGE } else { 0 }`.

`snapshot_for_entity` fills `durability`, `enchant_id`, `expires_tick`.

Quest mail: already in `Mailbox::send` from Task 4. Add a sim-level test `mail_refuses_quest_item` using `boar_tusk` in bags.

Update every `TickSnapshot {` literal in `woc-sim` tests to include `mail_postage: 0` if they construct snapshots by hand.

- [ ] **Step 4: Run** `cargo test -p woc-sim --lib -- --nocapture`

Expected: PASS including `tick_phase_order_fingerprint_locked`

- [ ] **Step 5: Commit**

```bash
git add crates/woc-sim/src/host.rs crates/woc-sim/src/sim.rs crates/woc-sim/src/mail.rs
git commit -m "feat(sim): wire mail return, postage snapshot, and tick expiry"
```

---

### Task 8: Client bank any non-quest stack; mail compose / send / collect / return

**Files:**
- Modify: `crates/woc-client/src/hud.rs`
- Modify: `crates/woc-client/src/input.rs`
- Modify: `crates/woc-client/src/world_setup.rs` (help footer string)

**Interfaces:**
- Consumes: `MailReturn`, `MailSend`, `mail_postage`, `first_bankable_bag_stack`
- Produces: mutually exclusive K/I; compose buffer; keys as spec §5.8

- [ ] **Step 1: Failing HUD tests** in `hud.rs` (extend `chrome_snapshot` with `mail_postage: 1` and a worn bank stack):

```rust
#[test]
fn bank_panel_offers_first_non_quest_deposit() {
    let text = bank_panel_text(&chrome_snapshot());
    assert!(text.contains("[G] Deposit"));
    assert!(!text.contains("first bag junk"));
}

#[test]
fn mail_panel_shows_send_and_numbered_collect() {
    let text = mail_panel_text(&chrome_snapshot());
    assert!(text.contains("[S] Send item"));
    assert!(text.contains("[P] Collect first mail"));
    assert!(text.contains("[1–9] Collect numbered"));
    assert!(text.contains("[X] Return"));
}
```

Change `first_junk_bag_stack` into `first_bankable_bag_stack` that skips `ItemKind::Quest`. Keep `first_junk_bag_stack` as a wrapper **only if** other call sites still need junk; otherwise replace all uses (bank **G** is the only one).

- [ ] **Step 2: Run** `cargo test -p woc-client bank_panel_offers_first_non_quest_deposit -- --nocapture`

Expected: FAIL (`woc-client` tests that compile without GPU)

- [ ] **Step 3: Implement**

`UiFlags` gains `mail_to: String`, `mail_compose: bool`. Default empty/false.

Opening **K** sets `show_mail = false` (and clears compose). Opening **I** sets `show_bank = false`. Opening **I** seeds `mail_to` from the snapshot entity whose id is `target_id` and `kind == Player` (use `e.name`).

`collect_intent`: if `ui.mail_compose` { do not set WASD/S movement from those keys }.

`handle_interact_keys`:

- Mail **Enter** toggles `mail_compose`.
- Mail **Esc** (when compose): blur compose; do not steal the global Esc target-clear if you can check compose first in `grab_cursor` / interact. If Esc already closes nothing for mail, handle compose blur at the start of `handle_interact_keys`.
- Subscribe to `EventReader<KeyboardInput>` in a new `mail_compose_text` system (copy the alphanumeric filter from `login.rs`, max 24). Only run when `mail_compose`.
- **S** while mail open (compose or not): `to_name = mail_to if !empty else target player name`; if still empty, local toast `"No recipient."`; else `MailSend` with `first_bankable_bag_stack` (item) or toast `"Nothing to send."` if neither item nor (for S) stack. **S** is item-only per spec.
- **Y** while mail open and bank closed: `MailSend { copper: snapshot.progress.copper.saturating_sub(host.snapshot.mail_postage), bag_slot: None, count: 0, to_name }`.
- **P** first mail collect (existing).
- **1–9** while mail open: collect `snap.mail.get(idx)`.
- **X** while mail open: `MailReturn` first mail id.

Bank panel text: `[G] Deposit {count}×{id} (first bag stack)` using `first_bankable_bag_stack`. Show `dur` / enchant on bank lines when present.

Mail panel: recipient line `To: {mail_to}_` when compose (cursor) else `To: {mail_to}`. Help line from spec §5.8. Postage from `snap.mail_postage`.

Footer in `world_setup.rs`: keep `K bank` / `I mail`; add `(S/Y send, P/X collect)` only if the string stays one line — otherwise leave the footer and rely on the panel help.

- [ ] **Step 4: Run** `cargo test -p woc-client -- --nocapture` and `cargo check -p woc-client`

Expected: PASS / check green

- [ ] **Step 5: Commit**

```bash
git add crates/woc-client/src/hud.rs crates/woc-client/src/input.rs crates/woc-client/src/world_setup.rs
git commit -m "feat(client): bank any stack; mail compose, send, numbered collect, return"
```

---

### Task 9: Server loads mailbox directory on realm boot

**Files:**
- Modify: `crates/woc-server/src/game_ws.rs`

**Interfaces:**
- Consumes: `Persist::list_mailbox_directory`, `Sim.directory.register`
- Produces: realm boot registers every persist character name

- [ ] **Step 1: Failing test** in `game_ws.rs` tests if a realm helper is reachable; otherwise test via `build_shared` equivalent unit on a new `fn load_directory(sim: &mut Sim, names: Vec<(String, Uuid)>)` in `bridge.rs`:

```rust
pub fn apply_mailbox_directory(sim: &mut Sim, names: &[(String, uuid::Uuid)]) {
    for (name, id) in names {
        sim.directory.register(name, id.to_string());
    }
}
```

Test in `bridge.rs` or `game_ws.rs`:

```rust
#[test]
fn directory_lookup_after_apply() {
    let mut sim = Sim::new_empty_eastbrook();
    let id = uuid::Uuid::nil();
    apply_mailbox_directory(&mut sim, &[("Ada".into(), id)]);
    let key = id.to_string();
    assert_eq!(sim.directory.lookup("ada"), Some(key.as_str()));
}
```

`lookup` returns `&str` — compare to `id.to_string()` via a let binding.

- [ ] **Step 2: Run** `cargo test -p woc-server directory_lookup_after_apply -- --nocapture`

Expected: FAIL until helper exists

- [ ] **Step 3: Implement** `apply_mailbox_directory` in `bridge.rs`. In `build_shared`:

```rust
match persist.list_mailbox_directory().await {
    Ok(names) => apply_mailbox_directory(&mut sim, &names),
    Err(e) => tracing::warn!("failed to load mailbox directory: {e}"),
}
```

- [ ] **Step 4: Run** `cargo test -p woc-server -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/woc-server/src/bridge.rs crates/woc-server/src/game_ws.rs
git commit -m "feat(server): load character mailbox directory at realm boot"
```

---

### Task 10: Docs, version tag, demo

**Files:**
- Modify: `VERSION.toml` (`rewrite_version = "1.14.0"`, `parity_target = "parcel-bank"`)
- Modify: `Cargo.toml` workspace.package.version `1.14.0`
- Modify: crate `woc-version` constants if they duplicate `VERSION.toml`
- Modify: `CHANGELOG.md`, `docs/ROADMAP.md`, `docs/parity/STATUS.md`, `docs/parity/DEMO.md`
- Modify: spec status line to Implemented

**Interfaces:**
- Consumes: §6 DoD
- Produces: tagged rewrite identity

- [ ] **Step 1: Grep** `1.13.0` and `gear-slots` in `VERSION.toml`, `crates/woc-version`, `docs/parity/STATUS.md` footer/DEMO, `docs/ROADMAP.md` table row.

- [ ] **Step 2: Update copy**

ROADMAP table: add

```markdown
| **1.14.0** (this branch) | `parcel-bank` | Instance-preserving bank/mail, offline parcels, client send |
```

Mark `1.13.0` shipped.

STATUS: new `parcel-bank` subsection, all rows `done` matching spec §6. Demo step 6 becomes:

```markdown
6. Bank a worn enchanted sword (K, G) and copper; mail a herb to an offline name (I, type, S); collect (P); list then buy/cancel on the AH; gather + craft a salve or copper shortsword.
```

CHANGELOG `1.14.0` Added bullets from the spec goal.

Bump `woc-version` rewrite string so the client footer reads `WoC-rs 1.14.0`.

Set spec status to `Implemented (1.14.0)`.

- [ ] **Step 3: Run** `cargo test --workspace --exclude woc-client` and `cargo check -p woc-client`

Expected: all PASS / check green. Fingerprint still `3214741777866168171`. `PROTOCOL_REV == 8`.

- [ ] **Step 4: Commit**

```bash
git add VERSION.toml Cargo.toml crates/woc-version docs CHANGELOG.md
git commit -m "docs: tag 1.14.0 parcel-bank"
```

---

## Self-review

**Spec coverage:** §5.1 helpers → Task 1. Bank + repair → Task 2. Protocol → Task 3. Directory/send/return/expire data → Task 4. Persist/directory API → Task 5. AH instance → Task 6. Tick/host/snapshot → Task 7. Client → Task 8. Server boot → Task 9. DoD docs/version → Task 10. Non-goals (NPC gate, COD, bag overflow) have no tasks.

**Placeholder scan:** none. Toast copy, constants, and signatures are locked.

**Type consistency:** `take_from_slot` / `put_stack` / `CharacterDirectory::register|lookup` / `MailAttachment` / `Mailbox::send(..., now_tick, directory)` / `deliver_system(..., Option<MailAttachment>)` / `MAIL_POSTAGE` / `MAIL_INBOX_CAP` / `MAIL_TTL_TICKS` / `InteractAction::MailReturn` match across tasks.

**Compile order:** Task 4 may need the `deliver_system` wrapper so market still builds; Task 6 removes the wrapper. Task 3 must land before Task 7 (`MailReturn` variant). Task 5 listing DTO fields may land before Task 6 `Listing` fields — map `None` until Task 6.
